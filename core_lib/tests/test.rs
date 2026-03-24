#[cfg(test)]
mod test {
    use std::f32::consts::PI;

    #[test]
    fn generate_random_sample() {
        for _ in 0..8 {
            print!("{},", rand::random::<f32>());
        }
        println!();
    }
    const SAMPLES: [f32; 8] = [0.33, 0.99, -0.26, 0.82, 0.47, -0.95, -0.67, 0.11];
    #[test]
    fn draw_cos_graph() -> anyhow::Result<()> {
        // let recording_stream = rerun::RecordingStreamBuilder::new("k=0").spawn()?;

        Ok(())
    }
    #[test]
    fn compute_test_frame() {
        for i in mdct_math_fn(&SAMPLES[0..4]) {
            print!("{},", i);
        }
        for i in mdct_math_fn(&SAMPLES[4..8]) {
            print!("{},", i);
        }
        println!();
    }
    fn mdct_math_fn(arr: &[f32]) -> Vec<f32> {
        let mut res = vec![0.0_f32; arr.len() / 2];
        for k in 0..(arr.len() / 2) {
            for (idx, n) in arr.iter().enumerate() {
                res[k] += *n
                    * (PI / 2.0 * (idx as f32 + 0.5 + arr.len() as f32 / 2.0) * (k as f32 + 0.5))
                        .cos();
            }
        }
        res
    }
}
