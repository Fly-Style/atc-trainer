//! StartScreen: server profile fields, trainer login, student join.

use crate::core::command::AppCommand;
use crate::ui::app::{AtcApp, Message, StartField};
use iced::widget::{button, column, container, horizontal_rule, row, text, text_input};
use iced::{Center, Element, Fill, Length};

pub fn view(app: &AtcApp) -> Element<'_, Message> {
    let server = column![
        text("Server").size(18),
        labelled(
            "HTTP base",
            text_input("http://127.0.0.1:8080", &app.state.server.http_base)
                .on_input(|v| Message::StartFieldChanged(StartField::HttpBase, v))
        ),
        labelled(
            "WS base",
            text_input("ws://127.0.0.1:8080", &app.state.server.ws_base)
                .on_input(|v| Message::StartFieldChanged(StartField::WsBase, v))
        ),
    ]
    .spacing(8);

    let trainer = column![
        text("Trainer").size(18),
        labelled(
            "Magic hash",
            text_input("trainer hash", &app.form.trainer_hash)
                .on_input(|v| Message::StartFieldChanged(StartField::TrainerHash, v))
        ),
        labelled(
            "Session name",
            text_input("e.g. Evening training", &app.form.session_name)
                .on_input(|v| Message::StartFieldChanged(StartField::SessionName, v))
        ),
        row![
            button("Login").on_press(Message::Command(AppCommand::LoginAsTrainer {
                hash: app.form.trainer_hash.clone()
            })),
            button("Create + open session")
                .on_press(Message::Command(AppCommand::CreateSession {
                    name: app.form.session_name.clone()
                })),
        ]
        .spacing(8),
    ]
    .spacing(8);

    let student = column![
        text("Student").size(18),
        labelled(
            "Session hash",
            text_input("join-XXXXXX", &app.form.student_hash)
                .on_input(|v| Message::StartFieldChanged(StartField::StudentHash, v))
        ),
        button("Join session").on_press(Message::Command(AppCommand::JoinAsStudent {
            session_hash: app.form.student_hash.clone()
        })),
    ]
    .spacing(8);

    let mut col = column![
        text("ATC Trainer").size(28),
        server,
        horizontal_rule(1),
        trainer,
        horizontal_rule(1),
        student,
    ]
    .spacing(16)
    .max_width(520);

    if let Some(err) = &app.state.last_error {
        col = col.push(text(format!("error: {err}")).color(iced::color!(0xCC4444)));
    }

    container(col)
        .center_x(Fill)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn labelled<'a>(
    label: &'a str,
    input: text_input::TextInput<'a, Message>,
) -> Element<'a, Message> {
    row![
        text(label).width(Length::Fixed(120.0)),
        input.padding(6).width(Length::Fill),
    ]
    .align_y(Center)
    .spacing(8)
    .into()
}
