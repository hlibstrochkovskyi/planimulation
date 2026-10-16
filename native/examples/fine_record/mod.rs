//! Diagnostic report binding, not security authentication or World identity.
use serde_json::{Value, json};
pub const INDEX_FORMAT: &str = "canonical-pretty-json-fnv1a64-1";
pub fn reference(name: &str, row: &Value) -> Result<Value, String> {
    if std::path::Path::new(name)
        .file_name()
        .and_then(|v| v.to_str())
        != Some(name)
    {
        return Err("Diagnostic row reference must be a local filename.".into());
    }
    let bytes = serde_json::to_vec_pretty(row).map_err(|e| e.to_string())?;
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    Ok(json!({"rowFile":name,"canonicalReportFnv1a64":format!("{hash:016x}")}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_index_binds_complete_values_and_local_name_but_is_not_security() {
        let row = json!({"bed":1.25,"history":[{"accepted":3.}],"timingSeconds":2.});
        let entry = reference("a.json", &row).unwrap();
        assert_eq!(entry, reference("a.json", &row).unwrap());
        assert!(reference("../a.json", &row).is_err());
        assert_ne!(entry, reference("b.json", &row).unwrap());
        let mut changed = row.clone();
        changed["history"][0]["accepted"] = json!(4.);
        assert_ne!(entry, reference("a.json", &changed).unwrap());
    }
}
