use dioxus::prelude::*;

use crate::{components::CardText, game::{Card, ColorSkin, GameState, Suit}};

#[component]
fn Emph(children: Element) -> Element {
    rsx! {
        strong {
            color: "#ff0",
            {children}
        }
    }
}

#[component]
pub fn Help(mut game_state: Signal<GameState>) -> Element {
    let st = game_state.read();
    let skin = st.skin;

    let stack_example = || {
        let mut ite = [
            Card { rank: 5, suit: Suit::Spades },
            Card { rank: 4, suit: Suit::Hearts },
            Card { rank: 3, suit: Suit::Clubs },
            Card { rank: 2, suit: Suit::Hearts },
        ].into_iter().map(|card| {
            rsx! {
                CardText { 
                    card, skin, color_mode: crate::game::ColorMode::Light,
                }
            }
        });


        let last = ite.next().unwrap();
        rsx! {
            {ite.next().unwrap()},
            for x in ite { "–", {x} },
            " can be placed on ", {last}
        }
    };

    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; font-size: 4.25rem; color: #fff; padding: 4rem;",
            class: "help",

            div {
                text_align: "left",
            }

            p {
                margin_top: "0",
                "The deck is a standard 52-card deck with 13 ranks and 4 suits."
            }

            p {
                "The 10 " Emph {"tableau"} " columns and the 4 " Emph {"free cells"} " are evenly divided among 2 " Emph {"wings"}
                ". The 4 " Emph {"foundation"} " stacks belong to neither wing."
            }

            p {
                "Cards in the tableau are stacked by " Emph{"decrementing ranks"} " and 
                    " Emph{"alternating color"} " (",
                    {if skin.colors != ColorSkin::FourColor {"red/black"} else {"warm/cool"}}
                    ,"). Such stacks of any size can be moved as a unit. (e.g. ",{stack_example()}")"
            }

            p {
                "Cards moving to a " Emph {"tableau"} " stack must come from the " Emph {"opposite wing"} ", with one exception:
                Cards moving to an empty tableau column may come from either wing."
            }

            p {
                "The " Emph {"free cells"} " hold one card each. Cards moving to a free cell must come from the " 
                Emph {"same wing"} "."
            }

            p {
                "To ",Emph{"win the game"},", stack all the cards to the " Emph{"foundations"} " in incrementing order by suit."
            }

            p {
                Emph{"Shortcut note:"}," Double-clicking on a card will try to move it to the foundations if possible, or a free cell otherwise."
            }
        }
    }
}