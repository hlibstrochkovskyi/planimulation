use planimulation_core::{Recipe, World, wire};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Read};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "camelCase", deny_unknown_fields)]
enum Command {
    Generate { recipe: Recipe },
    Advance { steps: u32 },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut world: Option<World> = None;
    loop {
        let mut line = Vec::new();
        let count = input.by_ref().take(32769).read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        if count > 32768 {
            return Err("Command exceeds 32 KiB".into());
        }
        let result = (|| -> Result<(), String> {
            let command: Command = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
            match command {
                Command::Generate { recipe } => {
                    let next = World::generate(recipe)?;
                    wire::snapshot(&mut output, &next).map_err(|e| e.to_string())?;
                    world = Some(next);
                }
                Command::Advance { steps } => {
                    let w = world.as_mut().ok_or("Generate a world first.")?;
                    w.advance(steps)?;
                    wire::frame(&mut output, w).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            wire::send(&mut output, json!({"kind":"error","message":error}), &[])?;
        }
    }
    Ok(())
}
