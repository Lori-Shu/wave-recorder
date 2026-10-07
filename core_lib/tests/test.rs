#[cfg(test)]
mod test {
    use std::{f32::consts::PI, fs::File, io::BufReader, path::PathBuf};

    use bitstream_io::{BitRead2, BitWrite2, LittleEndian};
    use hound::WavSpec;
    use image::{EncodableLayout, ExtendedColorType, RgbaImage};
    use my_audio_codec::codec::{TinyDecoder, TinyEncoder};
    use ndarray::ArrayViewMut2;

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
    #[test]
    fn bilinear_interpolation() {
        let dynamic_image = image::load(
            BufReader::new(File::open("vorbis_window.png").unwrap()),
            image::ImageFormat::Png,
        )
        .unwrap();
        let dynamic_image = dynamic_image.to_rgba8();
        let image_bytes = dynamic_image.as_bytes();
        let original_rect = [
            dynamic_image.width() as usize,
            dynamic_image.height() as usize,
        ];
        let target_rect = [1200, 600];
        let mut new_image_bytes = vec![0_u8; target_rect[0] * target_rect[1] * 4];
        for x_index in 0..target_rect[0] {
            for y_index in 0..target_rect[1] {
                let src_x = (x_index as f32) / target_rect[0] as f32;
                let src_y = (y_index as f32) / target_rect[1] as f32;
                let x = (src_x * original_rect[0] as f32).floor() as usize;
                let delta_x = src_x * original_rect[0] as f32 - x as f32;
                let y = (src_y * original_rect[1] as f32).floor() as usize;
                let delta_y = src_y * original_rect[1] as f32 - y as f32;

                let r =
                    calculate_pixel(&image_bytes, &original_rect, &x, &y, &delta_x, &delta_y, 0);
                let g =
                    calculate_pixel(&image_bytes, &original_rect, &x, &y, &delta_x, &delta_y, 1);
                let b =
                    calculate_pixel(&image_bytes, &original_rect, &x, &y, &delta_x, &delta_y, 2);
                let a =
                    calculate_pixel(&image_bytes, &original_rect, &x, &y, &delta_x, &delta_y, 3);
                new_image_bytes[x_index * 4 + y_index * target_rect[0] * 4] = r as u8;
                new_image_bytes[x_index * 4 + y_index * target_rect[0] * 4 + 1] = g as u8;
                new_image_bytes[x_index * 4 + y_index * target_rect[0] * 4 + 2] = b as u8;
                new_image_bytes[x_index * 4 + y_index * target_rect[0] * 4 + 3] = a as u8;
            }
        }
        image::save_buffer_with_format(
            "test1.avif",
            &new_image_bytes,
            target_rect[0] as u32,
            target_rect[1] as u32,
            ExtendedColorType::Rgba8,
            image::ImageFormat::Avif,
        )
        .unwrap();
    }
    fn calculate_pixel(
        image_bytes: &[u8],
        original_rect: &[usize],
        x: &usize,
        y: &usize,
        delta_x: &f32,
        delta_y: &f32,
        idx: usize,
    ) -> f32 {
        let x = (*x).min(original_rect[0] - 1);
        let y = (*y).min(original_rect[1] - 1);
        let x_next = (x + 1).min(original_rect[0] - 1);
        let y_next = (y + 1).min(original_rect[1] - 1);
        let q11 = image_bytes[(y * 4 * original_rect[0] as usize) + x * 4 + idx];
        let q21 = image_bytes[(y * 4 * original_rect[0] as usize) + x_next * 4 + idx];
        let q12 = image_bytes[(y_next * 4 * original_rect[0] as usize) + x * 4 + idx];
        let q22 = image_bytes[(y_next * 4 * original_rect[0] as usize) + x_next * 4 + idx];

        let p = (1.0 - delta_x) * (1.0 - delta_y) * q11 as f32
            + delta_x * (1.0 - delta_y) * q21 as f32
            + (1.0 - delta_x) * delta_y * q12 as f32
            + delta_x * delta_y * q22 as f32;
        p
    }
    struct YCbCr {
        y: u8,
        cb: u8,
        cr: u8,
        alpha: u8,
    }
    fn rgba_to_ycbcr(img: &RgbaImage) -> Vec<YCbCr> {
        img.pixels()
            .map(|pixel| {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let a = pixel[3];

                // 执行转换公式
                let y = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                let cb = (-0.169 * r - 0.331 * g + 0.5 * b + 128.0) as u8;
                let cr = (0.5 * r - 0.419 * g - 0.081 * b + 128.0) as u8;

                YCbCr {
                    y,
                    cb,
                    cr,
                    alpha: a,
                }
            })
            .collect()
    }
    fn ycbcr_to_rgba(y_cb_cr: &Vec<YCbCr>) -> Vec<u8> {
        let mut result = vec![];
        for item in y_cb_cr {
            let y = item.y as f32;
            let cb = item.cb as f32 - 128.0;
            let cr = item.cr as f32 - 128.0;

            let r = (y + 1.402 * cr).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344 * cb - 0.714 * cr).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * cb).clamp(0.0, 255.0) as u8;
            result.push(r);
            result.push(g);
            result.push(b);
            result.push(item.alpha);
        }
        result
    }
    #[test]
    fn hide_txt_in_image() {
        let dynamic_image = image::load(
            BufReader::new(
                File::open("D:/rustprojects/tiny-player/resources/background_0.png").unwrap(),
            ),
            image::ImageFormat::Png,
        )
        .unwrap();
        let dynamic_image = dynamic_image.to_rgba8();
        let chunks_width = dynamic_image.width();
        let chunks_height = dynamic_image.height();
        let mut dynamic_image = rgba_to_ycbcr(&dynamic_image);
        let y_bytes = dynamic_image
            .iter()
            .map(|item| item.y as f32)
            .collect::<Vec<f32>>();

        let mut array = ndarray::Array2::from_shape_vec(
            (chunks_height as usize, chunks_width as usize),
            y_bytes,
        )
        .unwrap();
        const TO_HIDE_TEXT: &str = "hello, world!";
        let mut array_mut_view = array.view_mut();
        let mut exact_chunks = array_mut_view
            .exact_chunks_mut((8, 8))
            .into_iter()
            .collect::<Vec<_>>();
        let mut dct_planner = rustdct::DctPlanner::<f32>::new();
        let plan_dct2 = dct_planner.plan_dct2(8);
        let plan_dct3 = dct_planner.plan_dct3(8);
        let mut scratch = vec![0.0_f32; 8];
        for item in &mut exact_chunks {
            for idx in 0..8 {
                let mut row_mut = item.row_mut(idx as usize);
                plan_dct2.process_dct2_with_scratch(row_mut.as_slice_mut().unwrap(), &mut scratch);
            }
            for idx in 0..8 {
                let mut col_mut = item.column_mut(idx as usize);
                let mut column_vec = col_mut.to_vec();
                plan_dct2.process_dct2_with_scratch(&mut column_vec, &mut scratch);
                col_mut.assign(&ndarray::Array1::from(column_vec));
            }
        }
        let str_bytes = TO_HIDE_TEXT.as_bytes();
        let total_bits = str_bytes.len() * 8;
        let mut bit_reader = bitstream_io::BitReader::<_, LittleEndian>::new(str_bytes);
        let mut index = 0;
        const Q_STEP: f32 = 10.0;
        {
            let le_bytes = (total_bits as u32).to_le_bytes();
            let mut total_bits_bit_reader =
                bitstream_io::BitReader::<_, LittleEndian>::new(&le_bytes[..]);
            loop {
                let read_result = total_bits_bit_reader.read_bit();
                if read_result.is_err() {
                    break;
                }
                if read_result.unwrap() {
                    let coefficient = exact_chunks[index].row_mut(0)[0];
                    let steps = (coefficient / Q_STEP).round();
                    if steps as u32 % 2 == 0 {
                        exact_chunks[index].row_mut(0)[0] = (steps + 1.0) * Q_STEP;
                    } else {
                        exact_chunks[index].row_mut(0)[0] = steps * Q_STEP;
                    }
                } else {
                    let coefficient = exact_chunks[index].row_mut(0)[0];
                    let steps = (coefficient / Q_STEP).round();
                    if steps as u32 % 2 == 0 {
                        exact_chunks[index].row_mut(0)[0] = steps * Q_STEP;
                    } else {
                        exact_chunks[index].row_mut(0)[0] = (steps + 1.0) * Q_STEP;
                    }
                }
                index += 1;
            }
        }
        println!("--------------------");
        loop {
            if index >= 8 * size_of::<u32>() + total_bits {
                break;
            }
            if bit_reader.read_bit().unwrap() {
                let coefficient = exact_chunks[index].row_mut(0)[0];
                let steps = (coefficient / Q_STEP).round();
                if steps as u32 % 2 == 0 {
                    exact_chunks[index].row_mut(0)[0] = (steps + 1.0) * Q_STEP;
                } else {
                    exact_chunks[index].row_mut(0)[0] = steps * Q_STEP;
                }
            } else {
                let coefficient = exact_chunks[index].row_mut(0)[0];
                let steps = (coefficient / Q_STEP).round();
                if steps as u32 % 2 == 0 {
                    exact_chunks[index].row_mut(0)[0] = steps * Q_STEP;
                } else {
                    exact_chunks[index].row_mut(0)[0] = (steps + 1.0) * Q_STEP;
                }
            }

            index += 1;
        }
        let mut scratch = vec![0.0_f32; 8];
        for factor_frame in &mut exact_chunks {
            for idx in 0..8 {
                let mut row_mut = factor_frame.row_mut(idx as usize);
                plan_dct3.process_dct3_with_scratch(row_mut.as_slice_mut().unwrap(), &mut scratch);
            }
            for idx in 0..8 {
                let mut col_mut = factor_frame.column_mut(idx as usize);
                let mut column_vec = col_mut.to_vec();
                plan_dct3.process_dct3_with_scratch(&mut column_vec, &mut scratch);
                col_mut.assign(&ndarray::Array1::from(column_vec));
            }
        }
        let y_bytes = rebuild_y_vec(&exact_chunks, chunks_width, chunks_height);
        const DCT_SCALE_2D: f32 = 16.0;
        let y_arr = y_bytes
            .iter()
            .map(|item| *item / DCT_SCALE_2D)
            .collect::<Vec<f32>>();
        let mut array =
            ndarray::Array2::from_shape_vec((chunks_height as usize, chunks_width as usize), y_arr)
                .unwrap();
        let mut array_mut_view = array.view_mut();
        let mut exact_chunks = array_mut_view
            .exact_chunks_mut((8, 8))
            .into_iter()
            .collect::<Vec<_>>();
        let mut dct_planner = rustdct::DctPlanner::<f32>::new();
        let plan_dct2 = dct_planner.plan_dct2(8);
        let mut scratch = vec![0.0_f32; 8];
        for item in &mut exact_chunks {
            for idx in 0..8 {
                let mut row_mut = item.row_mut(idx as usize);
                plan_dct2.process_dct2_with_scratch(row_mut.as_slice_mut().unwrap(), &mut scratch);
            }
            for idx in 0..8 {
                let mut col_mut = item.column_mut(idx as usize);
                let mut column_vec = col_mut.to_vec();
                plan_dct2.process_dct2_with_scratch(&mut column_vec, &mut scratch);
                col_mut.assign(&ndarray::Array1::from(column_vec));
            }
        }
        let mut string_buffer = Vec::with_capacity(4096);
        let mut hided_words_bytes =
            bitstream_io::BitWriter::<_, LittleEndian>::new(&mut string_buffer);
        let mut index = 0;
        // const Q_STEP: f32 = 4.0;
        let total_bit_length = {
            let mut total_bits_vec = Vec::with_capacity(4);
            let mut length_bit_writer =
                bitstream_io::BitWriter::<_, LittleEndian>::new(&mut total_bits_vec);
            let length_bits_len = size_of::<u32>() * 8;
            for _ in 0..length_bits_len {
                let val = exact_chunks[index].row(0)[0];
                let round = (val / Q_STEP).round();
                if round as u32 % 2 == 0 {
                    length_bit_writer.write_bit(false).unwrap();
                } else {
                    length_bit_writer.write_bit(true).unwrap();
                }
                index += 1;
            }
            println!("{}", total_bits_vec.len());
            u32::from_le_bytes(*(total_bits_vec.as_array().unwrap()))
        };
        println!("total bits{}", total_bit_length);
        loop {
            if index >= 8 * size_of::<u32>() + total_bit_length as usize {
                break;
            }
            let val = exact_chunks[index].row(0)[0];
            let round = (val / Q_STEP).round();
            if round as u32 % 2 == 0 {
                hided_words_bytes.write_bit(false).unwrap();
            } else {
                hided_words_bytes.write_bit(true).unwrap();
            }
            index += 1;
        }
        hided_words_bytes.byte_align().unwrap();
        hided_words_bytes.flush().unwrap();
        println!("{:?}", string_buffer.len());
        println!("{:?}", string_buffer);
        // for (idx, item) in dynamic_image.iter_mut().enumerate() {
        //     let normalized_y = y_bytes[idx] / DCT_SCALE_2D;
        //     item.y = (normalized_y).clamp(0.0, 255.0) as u8;
        // }
        // let rgba = ycbcr_to_rgba(&dynamic_image);
        // image::save_buffer_with_format(
        //     "test_hide1.png",
        //     &rgba,
        //     chunks_width,
        //     chunks_height,
        //     ExtendedColorType::Rgba8,
        //     image::ImageFormat::Png,
        // )
        // .unwrap();
    }
    fn rebuild_y_vec(chunks: &Vec<ArrayViewMut2<f32>>, width: u32, height: u32) -> Vec<f32> {
        let block_width = width / 8;
        let mut result = vec![0.0_f32; (width * height) as usize];
        for (idx, item) in chunks.iter().enumerate() {
            let block_pos = [idx % block_width as usize, idx / block_width as usize];
            let top_left_pos = [block_pos[0] * 8, block_pos[1] * 8];
            for x in 0..8 {
                for y in 0..8 {
                    result[(top_left_pos[1] + y) * width as usize + top_left_pos[0] + x] =
                        item.row(y)[x];
                }
            }
        }
        result
    }
    #[test]
    fn read_words_from_image() {
        let dynamic_image = image::load(
            BufReader::new(
                File::open("D:/rustprojects/my-audio-codec/core_lib/test_hide1.png").unwrap(),
            ),
            image::ImageFormat::Png,
        )
        .unwrap();
        let dynamic_image = dynamic_image.to_rgba8();
        let chunks_width = dynamic_image.width();
        let chunks_height = dynamic_image.height();
        let dynamic_image = rgba_to_ycbcr(&dynamic_image);
        let y_bytes = dynamic_image
            .iter()
            .map(|item| item.y as f32)
            .collect::<Vec<f32>>();

        let mut array = ndarray::Array2::from_shape_vec(
            (chunks_height as usize, chunks_width as usize),
            y_bytes,
        )
        .unwrap();
        let mut array_mut_view = array.view_mut();
        let mut exact_chunks = array_mut_view
            .exact_chunks_mut((8, 8))
            .into_iter()
            .collect::<Vec<_>>();
        let mut dct_planner = rustdct::DctPlanner::<f32>::new();
        let plan_dct2 = dct_planner.plan_dct2(8);
        let mut scratch = vec![0.0_f32; 8];
        for item in &mut exact_chunks {
            for idx in 0..8 {
                let mut row_mut = item.row_mut(idx as usize);
                plan_dct2.process_dct2_with_scratch(row_mut.as_slice_mut().unwrap(), &mut scratch);
            }
            for idx in 0..8 {
                let mut col_mut = item.column_mut(idx as usize);
                let mut column_vec = col_mut.to_vec();
                plan_dct2.process_dct2_with_scratch(&mut column_vec, &mut scratch);
                col_mut.assign(&ndarray::Array1::from(column_vec));
            }
        }
        let mut string_buffer = Vec::with_capacity(4096);
        let mut hided_words_bytes =
            bitstream_io::BitWriter::<_, LittleEndian>::new(&mut string_buffer);
        let mut index = 0;
        const Q_STEP: f32 = 10.0;
        let total_bit_length = {
            let mut total_bits_vec = Vec::with_capacity(4);
            let mut length_bit_writer =
                bitstream_io::BitWriter::<_, LittleEndian>::new(&mut total_bits_vec);
            let length_bits_len = size_of::<u32>() * 8;
            for _ in 0..length_bits_len {
                let val = exact_chunks[index].row(0)[0];
                let round = (val / Q_STEP).round();
                if round as u32 % 2 == 0 {
                    length_bit_writer.write_bit(false).unwrap();
                } else {
                    length_bit_writer.write_bit(true).unwrap();
                }
                index += 1;
            }
            length_bit_writer.flush().unwrap();
            println!("{:?}", total_bits_vec);
            u32::from_le_bytes(total_bits_vec.try_into().unwrap())
        };
        println!("total bits{}", total_bit_length);
        loop {
            if index >= 8 * size_of::<u32>() + total_bit_length as usize {
                break;
            }
            let val = exact_chunks[index].row(0)[0];
            let steps = (val / Q_STEP).round();
            if steps as u32 % 2 == 0 {
                hided_words_bytes.write_bit(false).unwrap();
            } else {
                hided_words_bytes.write_bit(true).unwrap();
            }
            index += 1;
        }
        hided_words_bytes.byte_align().unwrap();
        hided_words_bytes.flush().unwrap();
        println!("{:?}", string_buffer.len());
        println!("{:?}", string_buffer);
    }
    #[test]
    fn test_encode_to_gla() {
        for idx in 4..5 {
            let mut tiny_encoder = TinyEncoder::new(
                PathBuf::from("D:/rustprojects/my-audio-codec/core_lib"),
                44100,
                2,
            )
            .unwrap();
            let mut wav_reader =
                hound::WavReader::open(format!("D:/Downloads/test0{}.wav", idx)).unwrap();
            let collect = wav_reader
                .samples::<i16>()
                .map(|sample| (sample.unwrap() as f32) / i16::MAX as f32)
                .collect::<Vec<f32>>();
            tiny_encoder.reset_encoder().unwrap();
            tiny_encoder.encode(collect).unwrap();
            tiny_encoder.save_file().unwrap();
        }
    }
    #[test]
    fn test_decode_to_wav() {
        for idx in 4..5 {
            let mut tiny_decoder = TinyDecoder::new().unwrap();
            tiny_decoder
                .reset_input_file(PathBuf::from(format!(
                    "D:/rustprojects/my-audio-codec/core_lib/test0{}.gla",
                    idx
                )))
                .unwrap();
            tiny_decoder.read_file_header().unwrap();
            let mut wav_writer = hound::WavWriter::create(
                format!(
                    "D:/rustprojects/my-audio-codec/core_lib/test0{}_codec.wav",
                    idx
                ),
                WavSpec {
                    channels: 2,
                    sample_rate: 44100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            loop {
                if let Ok(frame) = tiny_decoder.pop_frame() {
                    for sample in frame.0 {
                        wav_writer
                            .write_sample((sample * i16::MAX as f32) as i16)
                            .unwrap();
                    }
                } else {
                    break;
                }
            }
        }
    }
}
mod figure {
    use my_audio_codec::AudioCodecResult;
    use plotters::{
        backend::PixelFormat,
        chart::{ChartBuilder, SeriesLabelPosition},
        coord::combinators::IntoLogRange,
        prelude::{BitMapBackend, IntoDrawingArea, PathElement, Rectangle, Text},
        series::LineSeries,
        style::{
            BLACK, BLUE, Color, IntoFont, RED, RGBAColor, ShapeStyle, SizeDesc, WHITE,
            text_anchor::Pos,
        },
    };
    use rustdct::{num_complex::Complex, num_traits::Zero, rustfft::FftPlanner};
    use std::{f64::consts::PI, path::Path};
    #[test]
    fn vorbis_figure() {
        let n_size = 1024;
        let out_file = "vorbis_window.png";

        // 1. 创建绘图后端 (使用 SVG 以满足论文矢量图要求)
        let root = BitMapBackend::new(out_file, (1920, 1080)).into_drawing_area();
        root.fill(&WHITE).unwrap(); // 论文通常要求白底

        // 2. 构建图表框架
        let mut chart = ChartBuilder::on(&root)
            .caption("Vorbis Window Function", ("sans-serif", 60).into_font())
            .margin(10)
            .x_label_area_size(100)
            .y_label_area_size(100)
            .build_cartesian_2d(0..n_size, 0f64..1.1f64)
            .unwrap(); // Y轴留一点余量

        // 3. 配置网格和轴标签
        chart
            .configure_mesh()
            .x_desc("Sample Index (n)")
            .y_desc("Amplitude w(n)")
            .axis_desc_style(("sans-serif", 50))
            .label_style(("sans-serif", 30))
            .draw()
            .unwrap();

        // 4. 计算并绘制 Vorbis 曲线
        // 使用 line_series 确保曲线连续
        chart
            .draw_series(LineSeries::new(
                (0..n_size).map(|n| {
                    let inner_sin = (PI / n_size as f64) * (n as f64 + 0.5);
                    let val = (PI / 2.0 * inner_sin.sin().powi(2)).sin();
                    (n, val)
                }),
                BLUE.stroke_width(6), // 论文常用深蓝色
            ))
            .unwrap()
            .label("Vorbis Window")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 50, y)], BLUE.stroke_width(6)));

        // 5. 绘制图例
        chart
            .configure_series_labels()
            .label_font(("sans-serif", 40).into_font())
            .legend_area_size(60)
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .unwrap();

        println!("成功生成论文级图表: {}", out_file);
    }
    #[test]
    fn residual_figure() {
        let sample_rate = 44100;
        let channels = 2;
        let out_file = "residual.png";
        let step_samples = sample_rate * channels / 2;
        let mut test01 = hound::WavReader::open("D:/Downloads/test01.wav")
            .unwrap()
            .into_samples::<i16>()
            .step_by(step_samples);
        let mut test01_codec =
            hound::WavReader::open("D:/rustprojects/my-audio-codec/core_lib/test01_codec.wav")
                .unwrap()
                .into_samples::<i16>()
                .skip(512)
                .step_by(step_samples);
        let test01_samples_len = test01.len().min(test01_codec.len());
        let test01_residuals = (0..test01_samples_len)
            .map(|_idx| {
                let residual = (test01.next().unwrap().unwrap() as f32) / i16::MAX as f32
                    - (test01_codec.next().unwrap().unwrap() as f32) / i16::MAX as f32;
                if residual.abs() > 2.0 {
                    println!("error sample > 1.0");
                }
                residual
            })
            .collect::<Vec<f32>>();
        let mut test02 = hound::WavReader::open("D:/Downloads/test02.wav")
            .unwrap()
            .into_samples::<i16>()
            .step_by(step_samples);
        let mut test02_codec =
            hound::WavReader::open("D:/rustprojects/my-audio-codec/core_lib/test02_codec.wav")
                .unwrap()
                .into_samples::<i16>()
                .skip(512)
                .step_by(step_samples);
        let test02_samples_len = test02.len().min(test02_codec.len());
        let test02_residuals = (0..test02_samples_len)
            .map(|_idx| {
                (test02.next().unwrap().unwrap() as f32) / i16::MAX as f32
                    - (test02_codec.next().unwrap().unwrap() as f32) / i16::MAX as f32
            })
            .collect::<Vec<f32>>();
        let mut test03 = hound::WavReader::open("D:/Downloads/test03.wav")
            .unwrap()
            .into_samples::<i16>()
            .step_by(step_samples);
        let mut test03_codec =
            hound::WavReader::open("D:/rustprojects/my-audio-codec/core_lib/test03_codec.wav")
                .unwrap()
                .into_samples::<i16>()
                .skip(512)
                .step_by(step_samples);
        let test03_samples_len = test03.len().min(test03_codec.len());
        let test03_residuals = (0..test03_samples_len)
            .map(|_idx| {
                (test03.next().unwrap().unwrap() as f32) / i16::MAX as f32
                    - (test03_codec.next().unwrap().unwrap() as f32) / i16::MAX as f32
            })
            .collect::<Vec<f32>>();
        let root = BitMapBackend::new(out_file, (1920, 1080)).into_drawing_area();
        root.fill(&RGBAColor(WHITE.0, WHITE.1, WHITE.2, 255.0))
            .unwrap();
        // println!("test01_samples_len:{}",test01_samples_len);
        // println!("test02_samples_len:{}",test02_samples_len);
        // println!("test03_samples_len:{}",test03_samples_len);
        let chart_samples_len = test01_samples_len
            .min(test02_samples_len)
            .min(test03_samples_len);
        let duration =
            ((chart_samples_len * step_samples) as f32) / (sample_rate as f32 * channels as f32);

        let mut chart = ChartBuilder::on(&root)
            .caption("Waveform Difference", ("sans-serif", 60).into_font())
            .margin(10)
            .x_label_area_size(120)
            .y_label_area_size(120)
            .build_cartesian_2d(0.0..duration, -0.02_f32..0.02_f32)
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            .x_desc("Time (s)")
            .y_desc("Amplitude")
            .axis_desc_style(("sans-serif", 50))
            .label_style(("sans-serif", 30))
            .x_label_formatter(&|x| format!("{}s", x))
            .draw()
            .unwrap();
        let mut test01_rmse = 0.0_f32;
        let test01_series = chart
            .draw_series(LineSeries::new(
                test01_residuals
                    .into_iter()
                    .take(chart_samples_len)
                    .enumerate()
                    .map(|n| {
                        let sample = n.1;
                        test01_rmse += sample * sample;
                        (
                            ((n.0 * step_samples) as f32) / (sample_rate as f32 * channels as f32),
                            sample,
                        )
                    }),
                ShapeStyle {
                    color: RGBAColor(BLUE.0, BLUE.1, BLUE.2, 180.0),
                    filled: false,
                    stroke_width: 4,
                },
            ))
            .unwrap();
        test01_rmse = (test01_rmse / chart_samples_len as f32).sqrt();
        test01_series
            .label(format!("Pop (RMSE:{:.4})", test01_rmse))
            .legend(|(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 50, y)],
                    ShapeStyle {
                        color: RGBAColor(BLUE.0, BLUE.1, BLUE.2, 180.0),
                        filled: false,
                        stroke_width: 4,
                    },
                )
            });
        let mut test02_rmse = 0.0_f32;
        let test02_series = chart
            .draw_series(LineSeries::new(
                test02_residuals
                    .into_iter()
                    .take(chart_samples_len)
                    .enumerate()
                    .map(|n| {
                        let sample = n.1;
                        test02_rmse += sample * sample;
                        (
                            ((n.0 * step_samples) as f32) / (sample_rate as f32 * channels as f32),
                            sample,
                        )
                    }),
                ShapeStyle {
                    color: RGBAColor(153, 0, 0, 180.0),
                    filled: false,
                    stroke_width: 4,
                },
            ))
            .unwrap();
        test02_rmse = (test02_rmse / chart_samples_len as f32).sqrt();

        test02_series
            .label(format!("Percussion (RMSE:{:.4})", test02_rmse))
            .legend(|(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 50, y)],
                    ShapeStyle {
                        color: RGBAColor(153, 0, 0, 180.0),
                        filled: false,
                        stroke_width: 4,
                    },
                )
            });
        let mut test03_rmse = 0.0_f32;
        let test03_series = chart
            .draw_series(LineSeries::new(
                test03_residuals
                    .into_iter()
                    .take(chart_samples_len)
                    .enumerate()
                    .map(|n| {
                        let sample = n.1;
                        test03_rmse += sample * sample;
                        (
                            ((n.0 * step_samples) as f32) / (sample_rate as f32 * channels as f32),
                            sample,
                        )
                    }),
                ShapeStyle {
                    color: RGBAColor(34, 139, 34, 180.0),
                    filled: false,
                    stroke_width: 4,
                },
            ))
            .unwrap();
        test03_rmse = (test03_rmse / chart_samples_len as f32).sqrt();
        test03_series
            .label(format!("Symphony (RMSE:{:.4})", test03_rmse))
            .legend(|(x, y)| {
                PathElement::new(
                    vec![(x, y), (x + 50, y)],
                    ShapeStyle {
                        color: RGBAColor(34, 139, 34, 180.0),
                        filled: false,
                        stroke_width: 4,
                    },
                )
            });
        chart
            .draw_series(LineSeries::new(
                (0..chart_samples_len).map(|idx| (idx as f32, 0.0f32)),
                RGBAColor(BLACK.0, BLACK.1, BLACK.2, 127.0).stroke_width(2),
            ))
            .unwrap();
        chart
            .configure_series_labels()
            .label_font(("sans-serif", 40).into_font())
            .legend_area_size(60)
            .background_style(&WHITE.mix(0.8))
            .border_style(BLACK.stroke_width(1))
            .position(SeriesLabelPosition::UpperLeft)
            .draw()
            .unwrap();
    }
    #[test]
    fn snr_figure() {
        let sample_rate = 44100;
        let channels = 2;
        let out_file = "snr_comparison.png";
        let step_samples = sample_rate * channels / 2;
        let codec_test01_snr = load_and_compute_snr(
            Path::new("D:/Downloads/test01.wav"),
            Path::new("D:/rustprojects/my-audio-codec/core_lib/test01_codec.wav"),
            step_samples,
        );
        let codec_test02_snr = load_and_compute_snr(
            Path::new("D:/Downloads/test02.wav"),
            Path::new("D:/rustprojects/my-audio-codec/core_lib/test02_codec.wav"),
            step_samples,
        );
        let codec_test03_snr = load_and_compute_snr(
            Path::new("D:/Downloads/test03.wav"),
            Path::new("D:/rustprojects/my-audio-codec/core_lib/test03_codec.wav"),
            step_samples,
        );
        let bit_map_backend = BitMapBackend::new(out_file, (1920, 1080));
        let drawing_area = bit_map_backend.into_drawing_area();
        drawing_area.fill(&WHITE).unwrap();
        let mut bar_chart = ChartBuilder::on(&drawing_area)
            .x_label_area_size(120)
            .y_label_area_size(120)
            .margin(20)
            .caption("Signal-to-Noise Ratio", ("sans-serif", 80).into_font())
            .build_cartesian_2d(0.0_f32..3.0_f32, 0.0_f32..40.0_f32)
            .unwrap();
        bar_chart
            .configure_mesh()
            .disable_mesh()
            .y_desc("Ratio (dB)")
            .x_desc("Music Signal Style")
            .label_style(("sans-serif", 40))
            .axis_desc_style(("sans-serif", 50))
            .x_label_formatter(&|x| match x {
                0.5 => "Pop".to_string(),
                1.5 => "Percussion".to_string(),
                2.5 => "Symphony".to_string(),
                _ => "".to_string(),
            })
            .draw()
            .unwrap();
        let codec_data = [
            (0, codec_test01_snr),
            (1, codec_test02_snr),
            (2, codec_test03_snr),
        ];
        bar_chart
            .draw_series(codec_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Rectangle::new(
                        [(*x as f32, 0.0), (*x as f32 + 0.9, *y)],
                        BLUE.filled().stroke_width(2),
                    ))
                } else {
                    None
                }
            }))
            .unwrap()
            .label("Proposed Codec")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x + 50, y)], BLUE.filled().stroke_width(6))
            });
        bar_chart
            .draw_series(codec_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Text::new(
                        format!("{}dB", (*y) as u32),
                        (*x as f32 + 0.4, *y + 2.0),
                        ("sans-serif", 40),
                    ))
                } else {
                    None
                }
            }))
            .unwrap();
        bar_chart
            .configure_series_labels()
            .label_font(("sans-serif", 40).into_font())
            .legend_area_size(60)
            .background_style(&WHITE.mix(0.8))
            .border_style(BLACK.stroke_width(1))
            .position(SeriesLabelPosition::UpperRight)
            .draw()
            .unwrap();
    }
    fn load_and_compute_snr(original_path: &Path, decoded_path: &Path, step_samples: usize) -> f32 {
        let original_samples = hound::WavReader::open(original_path)
            .unwrap()
            .into_samples::<i16>()
            .map(|item| (item.unwrap() as f32) / i16::MAX as f32);
        let decoded_samples = hound::WavReader::open(decoded_path)
            .unwrap()
            .into_samples::<i16>()
            .skip(512)
            .map(|item| (item.unwrap() as f32) / i16::MAX as f32);
        let mut test_idx = 0;
        original_samples
            .step_by(step_samples)
            .zip(decoded_samples.step_by(step_samples))
            .map(|item| {
                let original_sample = item.0;
                let decoded_sample = item.1;
                if test_idx < 10 {
                    // println!("ori:{},decoded:{}", original_sample, decoded_sample);
                    test_idx += 1;
                }
                (
                    original_sample * original_sample,
                    (original_sample - decoded_sample) * (original_sample - decoded_sample),
                )
            })
            .reduce(|item0, item1| (item0.0 + item1.0, item0.1 + item1.1))
            .map(|(i0, i1)| 10.0 * (i0 / i1).log10())
            .unwrap()
    }
    pub fn find_alignment_offset(orig: &[f32], decoded: &[f32]) -> isize {
        let n = orig.len() + decoded.len();

        // 找到 >= n 的最近 2 的幂
        let size = n.next_power_of_two();

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        let ifft = planner.plan_fft_inverse(size);

        // 构造复数输入
        let mut a: Vec<Complex<f32>> = vec![Complex::zero(); size];
        let mut b: Vec<Complex<f32>> = vec![Complex::zero(); size];

        for i in 0..orig.len() {
            a[i].re = orig[i];
        }
        for i in 0..decoded.len() {
            b[i].re = decoded[i];
        }

        // FFT
        fft.process(&mut a);
        fft.process(&mut b);

        // 互相关：A * conj(B)
        for i in 0..size {
            a[i] = a[i] * b[i].conj();
        }

        // IFFT
        ifft.process(&mut a);

        // 找最大值位置
        let mut max_idx = 0;
        let mut max_val = f32::MIN;

        for (i, c) in a.iter().enumerate() {
            if c.re > max_val {
                max_val = c.re;
                max_idx = i;
            }
        }

        // 转换为偏移（处理循环卷积）
        let offset = if max_idx > size / 2 {
            max_idx as isize - size as isize
        } else {
            max_idx as isize
        };

        offset
    }
    #[test]
    fn compression_comparison_figure() {
        let out_file = "compression_ratio_bar.png";
        let bit_map_backend = BitMapBackend::new(out_file, (1920, 1080));
        let drawing_area = bit_map_backend.into_drawing_area();
        drawing_area.fill(&WHITE).unwrap();
        let mut bar_chart = ChartBuilder::on(&drawing_area)
            .x_label_area_size(120)
            .y_label_area_size(120)
            .margin(20)
            .caption(
                "Compression Ratio Comparison",
                ("sans-serif", 80).into_font(),
            )
            .build_cartesian_2d(0.0_f32..3.0_f32, 0.0_f32..18.0_f32)
            .unwrap();
        bar_chart
            .configure_mesh()
            .disable_mesh()
            .bold_line_style(WHITE.mix(0.3))
            .y_desc("Compression Ratio")
            .x_desc("Music Style")
            .label_style(("sans-serif", 40))
            .axis_desc_style(("sans-serif", 50))
            .x_label_formatter(&|x| match x {
                0.5 => "Pop".to_string(),
                1.5 => "Percussion".to_string(),
                2.5 => "Symphony".to_string(),
                _ => "".to_string(),
            })
            .draw()
            .unwrap();
        let codec_data = [(0, 3.38), (1, 3.69), (2, 4.46)];
        let opus_data = [(0, 13.26), (1, 15.65), (2, 14.33)];
        bar_chart
            .draw_series(codec_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Rectangle::new(
                        [(*x as f32, 0.0), (*x as f32 + 0.4, *y)],
                        BLUE.filled().stroke_width(2),
                    ))
                } else {
                    None
                }
            }))
            .unwrap()
            .label("Proposed Codec")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x + 50, y)], BLUE.filled().stroke_width(16))
            });
        bar_chart
            .draw_series(codec_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Text::new(
                        format!("{}", *y),
                        (*x as f32 + 0.1, *y + 1.0),
                        ("sans-serif", 40),
                    ))
                } else {
                    None
                }
            }))
            .unwrap();
        bar_chart
            .draw_series(opus_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Rectangle::new(
                        [(*x as f32 + 0.5, 0.0), (*x as f32 + 0.9, *y)],
                        BLACK.filled().stroke_width(2),
                    ))
                } else {
                    None
                }
            }))
            .unwrap()
            .label("Opus")
            .legend(|(x, y)| {
                PathElement::new(vec![(x, y), (x + 50, y)], BLACK.filled().stroke_width(16))
            });
        bar_chart
            .draw_series(opus_data.iter().filter_map(|(x, y)| {
                if *x == 0 || *x == 1 || *x == 2 {
                    Some(Text::new(
                        format!("{}", *y),
                        (*x as f32 + 0.6, *y + 1.0),
                        ("sans-serif", 40),
                    ))
                } else {
                    None
                }
            }))
            .unwrap();
        // bar_chart
        //     .draw_series(
        //         Histogram::vertical(&bar_chart)
        //             .style(BLUE.filled().stroke_width(2))
        //             .data(
        //                 codec_data
        //                     .into_iter()
        //                     .map(|item| (item.0 as u32, item.1 as f32)),
        //             ),
        //     )
        //     .unwrap()
        //     .label("Proposed Codec")
        //     .legend(|(x, y)| {
        //         PathElement::new(vec![(x, y), (x + 50, y)], BLUE.filled().stroke_width(6))
        //     });
        // bar_chart
        //     .draw_series(
        //         Histogram::vertical(&bar_chart)
        //             .style(BLACK.filled().stroke_width(2))
        //             .data(
        //                 opus_data
        //                     .into_iter()
        //                     .map(|item| (item.0 as u32, item.1 as f32)),
        //             ),
        //     )
        //     .unwrap()
        //     .label("Opus")
        //     .legend(|(x, y)| {
        //         PathElement::new(vec![(x, y), (x + 50, y)], BLACK.filled().stroke_width(6))
        //     });
        bar_chart
            .configure_series_labels()
            .label_font(("sans-serif", 40).into_font())
            .legend_area_size(60)
            .background_style(&WHITE.mix(0.8))
            .border_style(BLACK.stroke_width(1))
            .position(SeriesLabelPosition::UpperRight)
            .draw()
            .unwrap();
        // let (title, content) = drawing_area.split_vertically(200);
        // title
        //     .titled(
        //         "Compression Ratio Original vs. Encoded",
        //         ("sans-serif", 80).into_font(),
        //     )
        //     .unwrap();
        // let (body, foot) = content.split_vertically(800);
        // let rows = body.split_evenly((7, 1));
        // body.draw(&PathElement::new(
        //     vec![(0, 0), (body.dim_in_pixel().0 as i32, 0)],
        //     ShapeStyle::from(&BLACK).stroke_width(2),
        // ))
        // .unwrap();
        // let split_evenly = rows[0].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Music Type",
        //         &TextStyle::from(("sans-serif", 60).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Codec",
        //         &TextStyle::from(("sans-serif", 60).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "Compression ratio",
        //         &TextStyle::from(("sans-serif", 60).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // rows[0]
        //     .draw(&PathElement::new(
        //         vec![
        //             (0, body.dim_in_pixel().1 as i32),
        //             (body.dim_in_pixel().0 as i32, body.dim_in_pixel().1 as i32),
        //         ],
        //         ShapeStyle::from(&BLACK).stroke_width(1),
        //     ))
        //     .unwrap();
        // let split_evenly = rows[1].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Pop",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Minimalist Implementation",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "12.24%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let split_evenly = rows[2].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Pop",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Opus",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "7.54%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let split_evenly = rows[3].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Percussion",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Minimalist Implementation",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "12.38%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let split_evenly = rows[4].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Percussion",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Opus",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "6.39%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let split_evenly = rows[5].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Symphony",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Minimalist Implementation",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "12.22%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let split_evenly = rows[6].split_evenly((1, 3));
        // let center_anchor = Pos::new(
        //     plotters::style::text_anchor::HPos::Center,
        //     plotters::style::text_anchor::VPos::Center,
        // );
        // let center_x = split_evenly[0].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[0].dim_in_pixel().1 / 2;
        // split_evenly[0]
        //     .draw_text(
        //         "Symphony",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[1].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[1].dim_in_pixel().1 / 2;
        // split_evenly[1]
        //     .draw_text(
        //         "Opus",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // let center_x = split_evenly[2].dim_in_pixel().0 / 2;
        // let center_y = split_evenly[2].dim_in_pixel().1 / 2;
        // split_evenly[2]
        //     .draw_text(
        //         "6.98%",
        //         &TextStyle::from(("sans-serif", 50).into_font())
        //             .pos(center_anchor)
        //             .color(&BLACK),
        //         (center_x as i32, center_y as i32),
        //     )
        //     .unwrap();
        // foot.draw(&PathElement::new(
        //     vec![(0, 0), (body.dim_in_pixel().0 as i32, 0)],
        //     ShapeStyle::from(&BLACK).stroke_width(2),
        // ))
        // .unwrap();
    }
    #[test]
    fn draw_ath_curve() -> AudioCodecResult<()> {
        let root = BitMapBackend::new("ath_curve.png", (1920, 1080)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(
                "Absolute Threshold of Hearing",
                ("sans-serif", 80).into_font(),
            )
            .margin(10)
            .x_label_area_size(120)
            .y_label_area_size(120)
            // 使用对数坐标轴更能反映听觉特性
            .build_cartesian_2d((20.0f32..20000.0f32).log_scale(), -10.0f32..80.0f32)?;

        chart
            .configure_mesh()
            .disable_mesh()
            .x_desc("Frequency (Hz)")
            .label_style(("sans-serif", 40))
            .axis_desc_style(("sans-serif", 50))
            .y_desc("Sound Pressure Level (dB SPL)")
            .draw()?;

        chart.draw_series(LineSeries::new(
            (20..20000).map(|f| f as f32).map(|f| {
                let khz = f / 1000.0;
                let ath = 3.64 * khz.powf(-0.8) - 6.5 * (-(khz - 3.3).powi(2) / 0.15).exp()
                    + 0.001 * khz.powi(4);
                (f, ath)
            }),
            RED.stroke_width(4),
        ))?;

        Ok(())
    }
}
