use std::sync::Arc;
use std::sync::{mpsc, RwLock};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::Decoder;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatReader;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use log::{debug, log_enabled, Level};

use dizi_lib::error::DiziResult;

use crate::audio::request::PlayerRequest;

use super::stream::StreamEvent;

pub fn decode_packets<T>(
    mut format: Box<dyn FormatReader>,
    mut decoder: Box<dyn Decoder>,
    track_id: u32,
) -> Option<Vec<T>>
where
    T: symphonia::core::sample::Sample
        + cpal::Sample
        + std::marker::Send
        + 'static
        + symphonia::core::conv::FromSample<i8>
        + symphonia::core::conv::FromSample<i16>
        + symphonia::core::conv::FromSample<i32>
        + symphonia::core::conv::FromSample<u8>
        + symphonia::core::conv::FromSample<u16>
        + symphonia::core::conv::FromSample<u32>
        + symphonia::core::conv::FromSample<f32>
        + symphonia::core::conv::FromSample<f64>
        + symphonia::core::conv::FromSample<symphonia::core::sample::i24>
        + symphonia::core::conv::FromSample<symphonia::core::sample::u24>,
{
    let mut channel_data: Option<Vec<T>> = None;

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(SymphoniaError::IoError(_)) => {
                break;
            }
            Err(err) => {
                // A unrecoverable error occured, halt decoding.
                eprintln!("{:?}", err);
                break;
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if decoded.frames() > 0 {
                    let spec = *decoded.spec();
                    let mut samples: SampleBuffer<T> =
                        SampleBuffer::new(decoded.frames() as u64, spec);
                    samples.copy_interleaved_ref(decoded);
                    match channel_data.as_mut() {
                        Some(channels) => {
                            for sample in samples.samples() {
                                channels.push(*sample);
                            }
                        }
                        None => {
                            let channel_count = spec.channels.count();
                            let mut channels: Vec<T> = vec![];
                            for sample in samples.samples() {
                                channels.push(*sample);
                            }
                            channel_data = Some(channels);
                        }
                    }
                }
                // Consume the decoded audio samples (see below).
            }
            Err(SymphoniaError::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
            }
            Err(SymphoniaError::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }
    channel_data
}

pub fn stream_loop_f32(
    stream_tx: mpsc::Sender<StreamEvent>,
    device: &cpal::Device,
    config: &StreamConfig,
    packets: Vec<f32>,
) -> DiziResult<(Stream, mpsc::Sender<PlayerRequest>)> {
    let err_fn = |err| eprintln!("A playback error has occured! {}", err);

    let channels_len = packets.len();

    let (playback_loop_tx, playback_loop_rx) = mpsc::channel();

    let frame_index = Arc::new(RwLock::new(0));
    let volume = Arc::new(RwLock::new(1.0));

    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let offset = { *frame_index.read().unwrap() };
            let mut i = 0;
            if let Ok(msg) = playback_loop_rx.try_recv() {
                match msg {
                    PlayerRequest::SetVolume(new_volume) => {
                        let mut current_volume = volume.write().unwrap();
                        *current_volume = new_volume;
                    }
                    _ => {}
                }
            }
            if offset >= channels_len {
                return;
            }
            let current_volume = { *volume.read().unwrap() };

            for d in data {
                if offset + i >= channels_len {
                    let mut offset = frame_index.write().unwrap();
                    *offset = channels_len;
                    let _ = stream_tx.send(StreamEvent::StreamEnded);
                    break;
                }
                *d = packets[offset + i] * current_volume;
                i += 1;
            }
            {
                let mut offset = frame_index.write().unwrap();
                *offset += i;
            }
        },
        err_fn,
    )?;
    stream.play()?;
    Ok((stream, playback_loop_tx))
}

pub fn stream_loop_i16(
    stream_tx: mpsc::Sender<StreamEvent>,
    device: &cpal::Device,
    config: &StreamConfig,
    packets: Vec<i16>,
) -> DiziResult<(Stream, mpsc::Sender<PlayerRequest>)> {
    let err_fn = |err| eprintln!("A playback error has occured! {}", err);

    let channels_len = packets.len();

    let (playback_loop_tx, playback_loop_rx) = mpsc::channel();

    let frame_index = Arc::new(RwLock::new(0));
    let volume = Arc::new(RwLock::new(1.0));

    let stream = device.build_output_stream(
        config,
        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            let offset = { *frame_index.read().unwrap() };
            let mut i = 0;
            if let Ok(msg) = playback_loop_rx.try_recv() {
                match msg {
                    PlayerRequest::SetVolume(new_volume) => {
                        let mut current_volume = volume.write().unwrap();
                        *current_volume = new_volume;
                    }
                    _ => {}
                }
            }
            if offset >= channels_len {
                return;
            }
            let current_volume = { *volume.read().unwrap() };

            for d in data {
                if offset + i >= channels_len {
                    let mut offset = frame_index.write().unwrap();
                    *offset = channels_len;
                    let _ = stream_tx.send(StreamEvent::StreamEnded);
                    break;
                }
                *d = (packets[offset + i] as f32 * current_volume) as i16;
                i += 1;
            }
            {
                let mut offset = frame_index.write().unwrap();
                *offset += i;
            }
        },
        err_fn,
    )?;
    stream.play()?;
    Ok((stream, playback_loop_tx))
}

pub fn stream_loop_u16(
    stream_tx: mpsc::Sender<StreamEvent>,
    device: &cpal::Device,
    config: &StreamConfig,
    packets: Vec<u16>,
) -> DiziResult<(Stream, mpsc::Sender<PlayerRequest>)> {
    let err_fn = |err| eprintln!("A playback error has occured! {}", err);

    let channels_len = packets.len();

    let (playback_loop_tx, playback_loop_rx) = mpsc::channel();

    let frame_index = Arc::new(RwLock::new(0));
    let volume = Arc::new(RwLock::new(1.0));

    let stream = device.build_output_stream(
        config,
        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
            let offset = { *frame_index.read().unwrap() };
            let mut i = 0;
            if let Ok(msg) = playback_loop_rx.try_recv() {
                match msg {
                    PlayerRequest::SetVolume(new_volume) => {
                        let mut current_volume = volume.write().unwrap();
                        *current_volume = new_volume;
                    }
                    _ => {}
                }
            }
            if offset >= channels_len {
                return;
            }
            let current_volume = { *volume.read().unwrap() };

            for d in data {
                if offset + i >= channels_len {
                    let mut offset = frame_index.write().unwrap();
                    *offset = channels_len;
                    let _ = stream_tx.send(StreamEvent::StreamEnded);
                    break;
                }
                *d = (packets[offset + i] as f32 * current_volume) as u16;
                i += 1;
            }
            {
                let mut offset = frame_index.write().unwrap();
                *offset += i;
            }
        },
        err_fn,
    )?;
    stream.play()?;
    Ok((stream, playback_loop_tx))
}
