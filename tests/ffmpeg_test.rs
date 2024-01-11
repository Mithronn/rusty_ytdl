use std::io::Write;
use std::process::Stdio;
use tokio::{io::AsyncWriteExt, process::Command};

#[ignore]
#[tokio::test]
async fn ffmpeg_test() {
    use rusty_ytdl::{Video, VideoOptions, VideoQuality, VideoSearchOptions};

    let args: Vec<String> = vec![
        // input as stdin
        "-i".to_string(),
        // aliases of pipe:0
        "-".to_string(),
        // "-analyzeduration".to_string(),
        // "0".to_string(),
        // "-loglevel".to_string(),
        // "0".to_string(),
        "-f".to_string(),
        "wav".to_string(), // "mpegts" to include video
        // start audio filter
        "-af".to_string(),
        "aresample=48000,asetrate=48000*0.8,bass=g=30:f=110:w=0.3".to_string(),
        // start video filter
        "-vf".to_string(),
        "eq=brightness=150:saturation=2".to_string(),
        // pipe to stdout
        "pipe:1".to_string(),
    ];

    let url = "FZ8BxMU3BYc";

    let video_options = VideoOptions {
        quality: VideoQuality::Highest,
        filter: VideoSearchOptions::VideoAudio,
        ..Default::default()
    };

    let video = Video::new_with_options(url, video_options).unwrap();
    let stream = video.stream().await.unwrap();

    let mut file = std::fs::File::create(r"./filter_applied_audio.wav").unwrap();

    let mut start_byte: Vec<u8> = vec![];
    let mut end_byte: usize = 0;

    while let Some(chunk) = stream.chunk().await.unwrap() {
        let cmd_output = cmd_run("ffmpeg", &args, &[&start_byte, chunk.as_slice()].concat()).await;

        let _ = file.write_all(&cmd_output[end_byte..]);

        if start_byte.is_empty() {
            start_byte = chunk;
            end_byte = cmd_output.len();
        }
    }
}

async fn cmd_run(cmd: impl Into<String>, args: &Vec<String>, data: &Vec<u8>) -> Vec<u8> {
    let mut cmd = Command::new(cmd.into());
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true);

    let mut process = cmd.spawn().expect("failed to execute ffmpeg");
    let mut stdin = process.stdin.take().expect("Failed to open stdin");
    let cloned_data = data.clone();
    tokio::spawn(async move { stdin.write_all(&cloned_data).await });

    let output = process
        .wait_with_output()
        .await
        .expect("Failed to read stdout");

    return output.stdout;
}
