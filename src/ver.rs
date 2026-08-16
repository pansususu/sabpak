use std::cmp::Ordering;

/// Comparación semántica de versiones numéricas (`1.2.10` > `1.2.9`).
/// Los segmentos no numéricos se ignoran (tratados como 0).
pub fn cmp(a: &str, b: &str) -> Ordering {
    let pa: Vec<i64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<i64> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).unwrap_or(&0);
        let y = pb.get(i).unwrap_or(&0);
        if x != y {
            return x.cmp(y);
        }
    }
    Ordering::Equal
}