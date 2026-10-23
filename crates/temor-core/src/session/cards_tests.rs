use super::cards::*;
use crate::vocab::ids::BlockId;

const B: BlockId = BlockId(7);
const S: SurfaceId = SurfaceId(3);

fn live() -> Cards {
    card_step(Cards::default(), CardIn::Adopted { id: B, session: S }).0
}

fn run(state: Cards, inputs: Vec<CardIn>) -> (Cards, Vec<CardOut>) {
    inputs
        .into_iter()
        .fold((state, Vec::new()), |(state, mut all), input| {
            let (state, outs) = card_step(state, input);
            all.extend(outs);
            (state, all)
        })
}

#[test]
fn a_program_without_a_card_is_asked_to_detach_once_it_has_one_it_is_not() {
    let (cards, outs) = card_step(Cards::default(), CardIn::Detach(B));
    assert_eq!(outs, [CardOut::Detach(B)]);
    assert!(cards.is_empty());
    let (_, outs) = card_step(live(), CardIn::Detach(B));
    assert!(outs.is_empty());
}

#[test]
fn an_adopted_program_is_a_running_card_of_normal_size() {
    let cards = live();
    let surface = cards.get(B).expect("a card");
    assert_eq!(surface.session, S);
    assert_eq!(surface.size, CardSize::Normal);
    assert_eq!(surface.placed, Placement::Card);
    assert_eq!(surface.life, Life::Running);
}

#[test]
fn the_size_icon_goes_round_and_resizes_the_terminal_each_time() {
    let (cards, outs) = run(live(), vec![CardIn::CycleSize(B), CardIn::CycleSize(B)]);
    assert_eq!(cards.get(B).map(|s| s.size), Some(CardSize::Compact));
    let sizes: Vec<_> = outs
        .iter()
        .filter_map(|out| match out {
            CardOut::Resize { size, .. } => Some(*size),
            _ => None,
        })
        .collect();
    assert_eq!(sizes, [CardSize::Tall, CardSize::Compact]);
}

#[test]
fn setting_the_size_it_already_has_does_nothing() {
    let (_, outs) = card_step(
        live(),
        CardIn::SetSize {
            id: B,
            size: CardSize::Normal,
        },
    );
    assert!(outs.is_empty());
}

#[test]
fn a_card_that_is_not_there_ignores_every_input() {
    for input in [
        CardIn::CycleSize(B),
        CardIn::Pop(B),
        CardIn::Dock(B),
        CardIn::Terminate(B),
        CardIn::GraceOver(B),
        CardIn::Ended { id: B, exit: None },
        CardIn::Dismiss(B),
    ] {
        let (cards, outs) = card_step(Cards::default(), input);
        assert!(cards.is_empty() && outs.is_empty());
    }
}

#[test]
fn popping_out_opens_the_mini_window_and_docking_closes_it() {
    let (cards, outs) = card_step(live(), CardIn::Pop(B));
    assert_eq!(cards.get(B).map(|s| s.placed), Some(Placement::Mini));
    assert!(outs.contains(&CardOut::OpenMini { id: B, session: S }));
    let (_, again) = card_step(cards.clone(), CardIn::Pop(B));
    assert!(again.is_empty(), "already in the mini window");
    let (cards, outs) = card_step(cards, CardIn::Dock(B));
    assert_eq!(cards.get(B).map(|s| s.placed), Some(Placement::Card));
    assert!(outs.contains(&CardOut::CloseMini(B)));
}

#[test]
fn terminate_interrupts_first_and_hangs_up_only_if_the_program_is_still_there() {
    let (cards, outs) = card_step(live(), CardIn::Terminate(B));
    assert_eq!(cards.get(B).map(|s| s.life), Some(Life::Stopping));
    assert!(outs.contains(&CardOut::Interrupt(S)) && outs.contains(&CardOut::StartGrace(B)));
    assert!(!outs.contains(&CardOut::HangUp(S)));
    let (_, again) = card_step(cards.clone(), CardIn::Terminate(B));
    assert!(again.is_empty(), "a second click does not interrupt again");
    let (_, outs) = card_step(cards.clone(), CardIn::GraceOver(B));
    assert_eq!(outs, [CardOut::HangUp(S)]);
    let (gone, _) = card_step(
        cards,
        CardIn::Ended {
            id: B,
            exit: Some(130),
        },
    );
    let (_, outs) = card_step(gone, CardIn::GraceOver(B));
    assert!(outs.is_empty(), "it left within the grace");
}

#[test]
fn a_program_that_leaves_keeps_its_last_frame_and_comes_back_from_the_mini_window() {
    let (cards, outs) = run(
        live(),
        vec![
            CardIn::Pop(B),
            CardIn::Ended {
                id: B,
                exit: Some(0),
            },
        ],
    );
    let surface = cards.get(B).expect("the card stays");
    assert_eq!(surface.life, Life::Ended(Some(0)));
    assert_eq!(surface.placed, Placement::Card);
    assert!(outs.contains(&CardOut::CloseMini(B)) && outs.contains(&CardOut::Freeze(B)));
}

#[test]
fn an_ended_card_cannot_be_resized_or_popped_and_is_put_away_on_dismiss() {
    let (cards, _) = card_step(live(), CardIn::Ended { id: B, exit: None });
    let (cards, outs) = run(cards, vec![CardIn::CycleSize(B), CardIn::Pop(B)]);
    assert!(outs.is_empty());
    let (cards, _) = card_step(cards, CardIn::Dismiss(B));
    assert!(cards.is_empty());
}

#[test]
fn a_running_card_is_not_dismissed() {
    let (cards, outs) = card_step(live(), CardIn::Dismiss(B));
    assert!(cards.get(B).is_some() && outs.is_empty());
}
