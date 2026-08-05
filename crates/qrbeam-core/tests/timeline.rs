use qrbeam_core::error::ProtocolError;
use qrbeam_core::timeline::{ChannelRequest, SymbolKind, Timeline};

fn one_symbol_channel(profile_id: u8) -> ChannelRequest {
    ChannelRequest {
        channel_id: 0,
        profile_id,
        symbols_per_frame: 1,
    }
}

#[test]
fn channels_in_one_tick_share_global_index_and_use_different_symbols() {
    let mut timeline = Timeline::new([0x55; 16], 9, vec![4, 4, 2]).unwrap();

    let plans = timeline
        .next_tick(&[
            ChannelRequest {
                channel_id: 0,
                profile_id: 1,
                symbols_per_frame: 1,
            },
            ChannelRequest {
                channel_id: 1,
                profile_id: 2,
                symbols_per_frame: 2,
            },
        ])
        .unwrap();

    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].global_frame_index, 0);
    assert_eq!(plans[0].global_frame_index, plans[1].global_frame_index);
    assert_ne!(plans[0].channel_id, plans[1].channel_id);
    assert_ne!(plans[0].first_symbol_id, plans[1].first_symbol_id);
    assert_eq!(plans[0].kind, SymbolKind::Source);
}

#[test]
fn newly_generated_ticks_have_monotonic_global_indices() {
    let mut timeline = Timeline::new([1; 16], 3, vec![10]).unwrap();
    let channel = one_symbol_channel(0);

    let first = timeline.next_tick(&[channel]).unwrap();
    let second = timeline.next_tick(&[channel]).unwrap();

    assert_eq!(first[0].global_frame_index, 0);
    assert_eq!(second[0].global_frame_index, 1);
}

#[test]
fn seek_back_saturates_at_zero_and_replays_exact_history() {
    let mut timeline = Timeline::new([2; 16], 4, vec![200]).unwrap();
    let channel = one_symbol_channel(0);
    for _ in 0..150 {
        timeline.next_tick(&[channel]).unwrap();
    }

    assert_eq!(timeline.seek_back(100), 50);
    assert_eq!(timeline.seek_back(100), 0);
    let replayed = timeline.next_tick(&[one_symbol_channel(3)]).unwrap();

    assert_eq!(replayed[0].global_frame_index, 0);
    assert_eq!(replayed[0].profile_id, 0);
    assert_eq!(replayed[0].first_symbol_id, 0);
}

#[test]
fn seek_segment_selects_its_most_recent_historical_tick() {
    let mut timeline = Timeline::new([3; 16], 5, vec![2, 2, 2]).unwrap();
    let channel = one_symbol_channel(0);
    for _ in 0..6 {
        timeline.next_tick(&[channel]).unwrap();
    }

    assert_eq!(timeline.seek_segment(1).unwrap(), 3);
    let replayed = timeline.next_tick(&[channel]).unwrap();

    assert_eq!(replayed[0].global_frame_index, 3);
    assert_eq!(replayed[0].segment_index, 1);
}

#[test]
fn repair_mode_generates_new_unique_esi_only_for_selected_segment() {
    let mut timeline = Timeline::new([4; 16], 6, vec![2, 3]).unwrap();
    timeline.start_repair(1).unwrap();
    let channels = [
        ChannelRequest {
            channel_id: 0,
            profile_id: 1,
            symbols_per_frame: 2,
        },
        ChannelRequest {
            channel_id: 1,
            profile_id: 2,
            symbols_per_frame: 1,
        },
    ];

    let first = timeline.next_tick(&channels).unwrap();
    let second = timeline.next_tick(&channels).unwrap();

    assert!(first.iter().all(|plan| plan.segment_index == 1));
    assert!(first.iter().all(|plan| plan.kind == SymbolKind::Repair));
    assert_eq!(first[0].first_symbol_id, 3);
    assert_eq!(first[1].first_symbol_id, 5);
    assert_eq!(second[0].first_symbol_id, 6);
    assert_eq!(second[1].first_symbol_id, 8);
}

#[test]
fn stopping_repair_mode_continues_the_source_schedule() {
    let mut timeline = Timeline::new([5; 16], 7, vec![2, 2]).unwrap();
    timeline.start_repair(1).unwrap();
    let repair = timeline.next_tick(&[one_symbol_channel(0)]).unwrap();
    timeline.stop_repair();
    let source = timeline.next_tick(&[one_symbol_channel(1)]).unwrap();

    assert_eq!(repair[0].kind, SymbolKind::Repair);
    assert_eq!(source[0].kind, SymbolKind::Source);
    assert_eq!(source[0].segment_index, 0);
    assert_eq!(source[0].first_symbol_id, 0);
}

#[test]
fn profile_changes_do_not_change_file_session_identity() {
    let mut timeline = Timeline::new([0x77; 16], 88, vec![8]).unwrap();

    let stable = timeline.next_tick(&[one_symbol_channel(0)]).unwrap();
    let extreme = timeline.next_tick(&[one_symbol_channel(3)]).unwrap();

    assert_eq!(stable[0].session_id, extreme[0].session_id);
    assert_eq!(stable[0].file_id, extreme[0].file_id);
    assert_eq!(stable[0].profile_id, 0);
    assert_eq!(extreme[0].profile_id, 3);
}

#[test]
fn source_phase_transitions_to_round_robin_repair() {
    let mut timeline = Timeline::new([6; 16], 8, vec![1, 1]).unwrap();
    let channel = one_symbol_channel(0);

    let source_zero = timeline.next_tick(&[channel]).unwrap();
    let source_one = timeline.next_tick(&[channel]).unwrap();
    let repair_zero = timeline.next_tick(&[channel]).unwrap();
    let repair_one = timeline.next_tick(&[channel]).unwrap();

    assert_eq!(source_zero[0].segment_index, 0);
    assert_eq!(source_one[0].segment_index, 1);
    assert_eq!(repair_zero[0].segment_index, 0);
    assert_eq!(repair_one[0].segment_index, 1);
    assert_eq!(repair_zero[0].kind, SymbolKind::Repair);
    assert_eq!(repair_zero[0].first_symbol_id, 1);
}

#[test]
fn duplicate_channel_ids_are_rejected() {
    let mut timeline = Timeline::new([7; 16], 9, vec![4]).unwrap();
    let channel = one_symbol_channel(0);

    assert_eq!(
        timeline.next_tick(&[channel, channel]),
        Err(ProtocolError::InvalidTimeline("duplicate channel ID"))
    );
}
