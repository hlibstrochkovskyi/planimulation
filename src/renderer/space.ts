import { AdditiveBlending, BackSide, BufferAttribute, BufferGeometry, DoubleSide, Group, LinearFilter, Matrix3, Matrix4, Mesh, Points, RepeatWrapping, ShaderMaterial, SphereGeometry, SRGBColorSpace, TextureLoader, Vector3 } from 'three';
import type { Camera } from 'three';
import type { Texture } from 'three';

function variation(index: number, channel: number): number {
  const value = Math.sin(index * 127.1 + channel * 311.7) * 43758.5453;
  return value - Math.floor(value);
}

/** Fixed visual scenery. No simulation random stream, model time, or physical field is used. */
export class SpaceBackdrop extends Group {
  private readonly stars: Points<BufferGeometry, ShaderMaterial>;
  private readonly atmosphere: Mesh<SphereGeometry, ShaderMaterial>;
  private readonly sky: Mesh<BufferGeometry, ShaderMaterial>;
  private readonly panorama: Texture;
  private panoramaReady = false;
  private skyEnabled = true;
  private disposed = false;
  private atmosphereEnabled = true;
  private surfaceMode = true;

  constructor(pixelRatio: number, skyLoaded: (ready: boolean) => void) {
    super();
    const count = 2600;
    const positions = new Float32Array(count * 3), brightness = new Float32Array(count), temperatures = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      const y = 1 - 2 * variation(i, 1), radial = Math.sqrt(1 - y * y), angle = variation(i, 2) * Math.PI * 2;
      positions.set([Math.cos(angle) * radial * 80, y * 80, Math.sin(angle) * radial * 80], i * 3);
      brightness[i] = variation(i, 3) ** 3;
      temperatures[i] = variation(i, 4);
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new BufferAttribute(positions, 3));
    geometry.setAttribute('brightness', new BufferAttribute(brightness, 1));
    geometry.setAttribute('temperature', new BufferAttribute(temperatures, 1));
    this.stars = new Points(geometry, new ShaderMaterial({
      transparent: true, depthWrite: false, uniforms: { pixelRatio: { value: pixelRatio } },
      vertexShader: `attribute float brightness; attribute float temperature;
        varying float strength; varying float warmth;
        uniform float pixelRatio;
        void main() { strength=brightness; warmth=temperature;
          gl_PointSize=(0.8+brightness*1.8)*pixelRatio;
          gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0); }`,
      fragmentShader: `varying float strength; varying float warmth;
        void main() { float radius=length(gl_PointCoord-0.5)*2.0;
          float profile=exp(-radius*radius*6.0)+0.12*exp(-radius*radius*2.8);
          float alpha=profile*(0.18+strength*0.75)*(1.0-smoothstep(0.75,1.0,radius));
          vec3 color=mix(vec3(0.77,0.84,0.96),vec3(0.98,0.90,0.78),warmth);
          gl_FragColor=vec4(color,alpha); }`,
    }));
    this.stars.renderOrder = -2;
    // Sky rays depend only on camera projection and rotation, never nearby geometry or translation.
    const backdrop = new BufferGeometry();
    backdrop.setAttribute('position', new BufferAttribute(new Float32Array([-1,-1,0, 3,-1,0, -1,3,0]), 3));
    this.sky = new Mesh(backdrop, new ShaderMaterial({
      side: DoubleSide, depthWrite: false, depthTest: false,
      uniforms: { panorama: { value: null }, exposure: { value: .10 },
        inverseProjection: { value: new Matrix4() }, cameraRotation: { value: new Matrix3() }, skyRotationInverse: { value: new Matrix3() } },
      vertexShader: `varying vec2 screenPoint;
        void main() { screenPoint=position.xy; gl_Position=vec4(position.xy,1.0,1.0); }`,
      fragmentShader: `uniform sampler2D panorama; uniform float exposure; varying vec2 screenPoint;
        uniform mat4 inverseProjection; uniform mat3 cameraRotation; uniform mat3 skyRotationInverse;
        void main() {
          vec4 projected=inverseProjection*vec4(screenPoint,1.0,1.0);
          vec3 viewRay=normalize(projected.xyz/projected.w);
          vec3 direction=normalize(skyRotationInverse*cameraRotation*viewRay);
          vec2 skyUv=vec2(atan(-direction.z,direction.x)*0.159154943091895+0.5,asin(clamp(direction.y,-1.0,1.0))*0.318309886183791+0.5);
          vec3 color=texture2D(panorama,skyUv).rgb;
          gl_FragColor=vec4(color*exposure,1.0);
          #include <colorspace_fragment>
        }`,
    }));
    this.sky.visible = false; this.sky.renderOrder = -10; this.sky.frustumCulled = false;
    // A fixed presentation orientation gives the galactic plane a quiet diagonal.
    const center = new Vector3(-.95, .15, -.27).normalize();
    const north = new Vector3(0, .85, .52).addScaledVector(center, -new Vector3(0, .85, .52).dot(center)).normalize();
    const east = new Vector3().crossVectors(center, north).normalize();
    this.sky.material.uniforms.skyRotationInverse.value.setFromMatrix4(new Matrix4().makeBasis(center, north, east)).transpose();
    this.panorama = new TextureLoader().load(new URL('./sky/starmap-2020-galactic.jpg', import.meta.url).href, texture => {
      if (this.disposed) { texture.dispose(); return; }
      this.panoramaReady = true; this.sky.visible = this.skyEnabled; this.stars.visible = false;
      skyLoaded(true);
    }, undefined, () => { if (!this.disposed) skyLoaded(false); });
    this.panorama.colorSpace = SRGBColorSpace; this.panorama.wrapS = RepeatWrapping;
    // Isotropic mip selection near a spherical pole stretches stars along the view ray.
    this.panorama.minFilter = LinearFilter; this.panorama.generateMipmaps = false;
    this.sky.material.uniforms.panorama.value = this.panorama;
    this.atmosphere = new Mesh(new SphereGeometry(1, 128, 96), new ShaderMaterial({
      side: BackSide, transparent: true, blending: AdditiveBlending, depthWrite: false, depthTest: false,
      uniforms: { groundRadius: { value: 1 }, outerRadius: { value: 1.025 }, scaleHeight: { value: .0025 },
        sunDirection: { value: new Vector3(-.4, .6, 1).normalize() } },
      vertexShader: `varying vec3 worldPoint;
        void main() { vec4 world=modelMatrix*vec4(position,1.0); worldPoint=world.xyz;
          gl_Position=projectionMatrix*viewMatrix*world; }`,
      fragmentShader: `uniform float groundRadius; uniform float outerRadius; uniform float scaleHeight;
        uniform vec3 sunDirection; varying vec3 worldPoint;
        vec2 intersectSphere(vec3 origin,vec3 ray,float radius) {
          float b=dot(origin,ray), discriminant=b*b-dot(origin,origin)+radius*radius;
          if(discriminant<0.0) return vec2(1e4,-1e4);
          float root=sqrt(discriminant); return vec2(-b-root,-b+root);
        }
        void main() {
          vec3 ray=normalize(worldPoint-cameraPosition);
          vec2 bounds=intersectSphere(cameraPosition,ray,outerRadius);
          float entry=max(0.0,bounds.x), exitPoint=bounds.y;
          vec2 ground=intersectSphere(cameraPosition,ray,groundRadius);
          if(ground.x>0.0 && ground.x<ground.y) exitPoint=min(exitPoint,ground.x);
          if(exitPoint<=entry) discard;
          float stepLength=(exitPoint-entry)/16.0;
          vec3 optical=vec3(0.0); float accumulated=0.0;
          vec3 coefficients=vec3(0.18,0.42,1.0)*5.0;
          for(int i=0;i<16;i++) {
            vec3 point=cameraPosition+ray*(entry+(float(i)+0.5)*stepLength);
            float height=max(0.0,length(point)-groundRadius);
            float density=exp(-height/scaleHeight);
            float sunCos=dot(normalize(point),sunDirection);
            float illuminated=smoothstep(-0.12,0.08,sunCos);
            float sunPath=scaleHeight/max(0.08,sunCos+0.12);
            vec3 transmittance=exp(-coefficients*(accumulated+sunPath*density));
            optical+=density*stepLength*illuminated*transmittance;
            accumulated+=density*stepLength;
          }
          float cosine=dot(ray,sunDirection), phase=0.75*(1.0+cosine*cosine);
          vec3 scattering=optical*coefficients*phase*0.55;
          gl_FragColor=vec4(scattering,1.0);
        }`,
    }));
    this.atmosphere.renderOrder = 2;
    this.setRadius(1); this.add(this.sky, this.stars, this.atmosphere);
  }

  setStars(visible: boolean): void {
    this.skyEnabled = visible; this.sky.visible = visible && this.panoramaReady;
    this.stars.visible = visible && !this.panoramaReady;
  }
  setAtmosphere(visible: boolean): void { this.atmosphereEnabled = visible; this.atmosphere.visible = visible && this.surfaceMode; }
  setSurfaceMode(surface: boolean): void { this.surfaceMode = surface; this.atmosphere.visible = surface && this.atmosphereEnabled; }
  setRadius(envelope: number, reference = 1): void {
    const scaleHeight = reference * .0025;
    const outerRadius = Math.max(reference + scaleHeight * 8, envelope + scaleHeight * 2);
    this.atmosphere.scale.setScalar(outerRadius);
    this.atmosphere.material.uniforms.groundRadius.value = reference;
    this.atmosphere.material.uniforms.outerRadius.value = outerRadius;
    this.atmosphere.material.uniforms.scaleHeight.value = scaleHeight;
  }
  updateView(camera: Camera, distance: number, nightBlend: number, worldSun: Vector3): void {
    // Stars are effectively at infinity: translation must not produce nearby parallax.
    this.stars.position.copy(camera.position);
    this.sky.material.uniforms.inverseProjection.value.copy(camera.projectionMatrixInverse);
    this.sky.material.uniforms.cameraRotation.value.setFromMatrix4(camera.matrixWorld);
    const scale = Math.max(0, Math.min(1, (distance - .4) / 5.6));
    this.sky.material.uniforms.exposure.value = .055 + .105 * scale * scale * (3 - 2 * scale);
    this.atmosphere.material.uniforms.sunDirection.value.set(-.4, .6, 1).normalize().transformDirection(camera.matrixWorld).lerp(worldSun, nightBlend).normalize();
  }
  dispose(): void {
    this.disposed = true; this.sky.geometry.dispose(); this.sky.material.dispose(); this.panorama.dispose();
    this.stars.geometry.dispose(); this.stars.material.dispose();
    this.atmosphere.geometry.dispose(); this.atmosphere.material.dispose();
  }
}
