use iced::{Subscription, Application, Command, Element, Font, Length, Settings, Theme};
use iced::widget::{tooltip, button, text_editor, container, column, text, row, horizontal_space};
use iced::executor;
use iced::theme;
use iced::highlighter::{self, Highlighter};
use std::io;
use iced::keyboard;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn main() -> iced::Result {
    Editor::run(Settings {
        default_font: Font::MONOSPACE,
        fonts: vec![include_bytes!("../fonts/save.ttf")
            .as_slice()
            .into()],

        ..Settings::default()
    })
}


struct Editor {
    path: Option<PathBuf>,
    content: text_editor::Content,
    error: Option<Error>,
    is_dirty: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),  
    Open,
    New,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    FileSaved(Result<PathBuf    , Error>),
    Save,
}

impl Application for Editor { 
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();


    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
            path: None, 
            content: text_editor::Content::new(),
            error: None,
            is_dirty: true,
        }, Command::perform(
            load_file(default_file()),
         Message::FileOpened,
            ),
        )
    }

    fn title(&self) -> String {
        String::from("Groovy Code")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Edit(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.error = None;
                self.content.edit(action);

                Command::none()
                }   
                Message::New => {
                    self.path = None;
                    self.content = text_editor::Content::new();
                    self.is_dirty = true;
                    Command::none()
                }
            Message::Open => Command::perform(pick_file(), Message::FileOpened),
            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.content = text_editor::Content::with(&content);

                Command::none()
            }
            Message::Save => {
                let text = self.content.text();

                Command::perform(save_file(self.path.clone(), text), Message::FileSaved)
            
            }
            Message::FileOpened(Ok((path, content))) => {
                    self.path = Some(path); 
                    self.is_dirty = false;
                    Command::none()
                }
            Message::FileSaved(Ok(path)) => {
                self.path = Some(path);
                self.is_dirty = false;
                Command::none()
            }
            Message::FileSaved(Err(error)) => {
                self.error = Some(error);
                Command::none()
            }
            Message::FileOpened(Err(error)) => {
                self.error = Some(error);

                Command::none()
                }
        }   

    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        keyboard::on_key_press(|keycode, modifiers|{
            match keycode {
                keyboard::KeyCode::S if modifiers.command() => Some(Message::Save),
                _ => None
            }
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let input = text_editor(&self.content)
        .on_edit(Message::Edit)
        .highlight::<Highlighter>(highlighter::Settings {
            theme: highlighter::Theme::SolarizedDark,
            extension: self
            .path
            .as_ref()
            .and_then(|path| path.extension()?.to_str())
            .unwrap_or("rs")
            .to_string()
        },
         |highlight, _theme |
            highlight.to_format(),
    );
        let controls = row![
            action(save_icon(), "Save File", self.is_dirty.then_some(Message::Save)),
            action(open_icon(),"Open File", Some(Message::Open)),
            action(new_icon(), "New File", Some(Message::New)),
        ]
        .spacing(10);
        let status_bar = {
                let status = if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                text(error.to_string())
        } else {
            match self.path.as_deref().and_then(Path::to_str) {
                Some(path) => text(path).size(20),
                None => text("New File  "),
            }
        };
    
            let position = {
                let (line, column) = self.content.cursor_position();
    
                text(format!("{}:{}", line + 1, column + 1))

             };

         row![status, horizontal_space(Length::Fill), position]

        };   

        container(column![controls, input, status_bar].spacing(10))
        .padding(10)
        .into() 
    }

    fn theme (&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

fn icon<'a, Message>(codepoint: char) -> Element<'static, Message> {
    const ICON_FONT: Font = Font::with_name("save");

    text(codepoint).font(ICON_FONT).into()

}

fn action<'a>(content: Element<'a, Message>, label: &str, on_press: Option<Message>) -> Element<'a, Message> {
    let is_disabled = on_press.is_none();
    tooltip(button(container(content).width(30).center_x()).on_press_maybe(on_press).padding([5, 10]).style (
        if is_disabled {theme::Button::Secondary} else {theme::Button::Primary}
    ), label, tooltip::Position::FollowCursor)
    .style(theme::Container::Box)
    .into()
}

fn new_icon<'a>() -> Element<'a, Message> {
    icon('\u{E800}')
}
fn save_icon<'a>() -> Element<'a, Message> {
    icon('\u{F115}')
}
fn open_icon<'a>() -> Element<'a, Message> {
    icon('\u{E801}')
}

fn default_file() -> PathBuf {
    PathBuf::from(format!("{}/src/main.rs", env!("CARGO_MANIFEST_DIR")))
}

async fn pick_file() -> Result<(PathBuf, Arc<String>), Error> {
    let handle = rfd::AsyncFileDialog::new()
    .set_title("Open File")
    .pick_file()
    .await
    .ok_or(Error::DialogClosed)?;

    load_file(handle.path().to_owned()).await


} 
async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path  {
        path
 } else { 
        rfd::AsyncFileDialog::new()
        .set_title("Save File")
        .save_file()
        .await
        .ok_or(Error::DialogClosed).map(|handle| handle.path().to_owned())?
    };


    tokio::fs::write(&path, &text)
    .await
    .map_err(|error| Error::IOFailed(error.kind()))?;

    Ok(path)
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let contents = tokio::fs::read_to_string(&path)
    .await
    .map(Arc::new)
    .map_err(|error| error.kind())
    .map_err(Error::IOFailed)?;

    Ok((path, contents))
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IOFailed(io::ErrorKind),
}