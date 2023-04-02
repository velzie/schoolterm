mod schooltool;
mod tui;
use chrono::{DateTime, Utc};
use console_engine::crossterm::event::{self, KeyEvent};
use console_engine::events::Event;
use console_engine::forms::FormField;

use console_engine::pixel::{self};

use console_engine::{Color, KeyCode, KeyModifiers};
use home;
use schooltool::{SchoolTool, Student};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::fmt::Display;
use std::fs;
use std::future::Future;
use std::path::PathBuf;

use std::thread::{self};
use std::time::UNIX_EPOCH;
use std::{error::Error, time::Duration};
use termsize::{self, Size};

use tokio::sync::mpsc::{self, Receiver};
use tokio::sync::oneshot;
use tui::{AsWidget, Rect, Theme, Tui, Widget};

pub const MARKINGPERIODIDS: [u16; 4] = [592, 591, 590, 589];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserData {
    baseurl: String,
    username: String,
    password: String,
    valid: bool,
}
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(home::home_dir().unwrap().join(".config"))?;
    let cfg_file = home::home_dir().unwrap().join(".config").join("schoolterm");
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let t1 = thread::spawn(move || {
        let mut userdata = get_userdata(&cfg_file).unwrap();

        tui_thread(&mut userdata, tx);
        fs::write(cfg_file, serde_json::to_string(&userdata).unwrap()).unwrap();
    });

    // top thread needs to be std::thread, bottom needs to be tokio. why? i don't fucking know
    tokio::spawn(async {
        net_thread(rx).await;
    });

    t1.join().unwrap();

    Ok(())
}
fn get_userdata(file: &PathBuf) -> Result<UserData, Box<dyn Error>> {
    let Ok(contents) = fs::read_to_string(file) else {return Ok(UserData::default())};
    Ok(serde_json::from_str(&contents)?)
}
#[derive(Debug)]
struct QuarterDataResponse {
    quarters: Vec<Quarter>,
    activequarter: Value,
    courses: Vec<Value>,
}
#[derive(Debug)]
struct Quarter {
    id: Value,
    name: String,
}
#[derive(Debug)]
enum Command {
    Login {
        username: String,
        password: String,
        baseurl: String,
        resp: Responder<Option<Student>>,
    },
    QuarterData {
        data_type: String,
        quarter: Value,
        resp: Responder<QuarterDataResponse>,
    },
}

#[derive(Debug)]
struct Exit {}
impl Error for Exit {}
impl Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "");
        Ok(())
    }
}

type Responder<T> = oneshot::Sender<T>;

fn tui_thread(userdata: &mut UserData, tx: mpsc::Sender<Command>) -> Result<(), Box<dyn Error>> {
    let theme = Theme {
        bg: Color::Rgb {
            r: 10,
            g: 10,
            b: 10,
        },
        bg_accent: Color::Black,
        fg: Color::DarkBlue,
        fg_accent: Color::Blue,
        font: Color::White,
    };

    let mut t = Tui::new()?;

    let mut student = None;
    while !student.is_some() {
        if !userdata.valid {
            t.userdata_form(userdata);
        }
        let (resp_tx, resp_rx) = oneshot::channel();

        tx.blocking_send(Command::Login {
            username: userdata.username.clone(),
            password: userdata.password.clone(),
            baseurl: userdata.baseurl.clone(),
            resp: resp_tx,
        })
        .unwrap();
        student = t.poll_blocking(resp_rx)?;

        if student.is_some() {
            userdata.valid = true;
        } else {
            userdata.valid = false;
        }
    }

    let student = student.unwrap();
    let mut quarters: Vec<Option<Vec<Quarter>>> = vec![None, None];
    let mut courses: Vec<Vec<Option<Vec<Value>>>> = vec![vec![None; 5]; 2];

    // each option has 1
    let mut should_resize = true;

    let mut profdisplay = tui::TextDisplay {
        rect: Rect::default(),
        text: format!(
            "Logged in as {}. Today is a day {}",
            student.name,
            student.cycle_day.unwrap_or_default()
        ),
    };
    let mut topdrawer = tui::Drawer {
        rect: Rect::default(),
        index: 0,
        clicked: false,
        clicked_index: 0,
        buttons: vec!["Exit".into(), "Log out".into(), "Change theme".into()],
    };
    let mut typedrawer = tui::Drawer {
        rect: Rect::default(),
        index: 0,
        clicked: false,
        clicked_index: 0,
        buttons: vec!["Assignments".into(), "Grades".into()],
    };
    let mut quarterdrawer = tui::Drawer {
        rect: Rect::default(),
        index: 0,
        clicked: false,
        clicked_index: 0,
        buttons: vec![],
    };
    let mut classdrawer = tui::Drawer {
        rect: Rect::default(),
        index: 0,
        clicked: false,
        clicked_index: 0,
        buttons: vec![],
    };
    let mut table = tui::Table {
        rect: Rect::default(),
        indecies: vec![],
        data: vec![],
        index: 0,
    };

    let mut selected_widget = 0;
    // let mut should_update_table = false;

    loop {
        if topdrawer.clicked {
            topdrawer.clicked = false;
            match topdrawer.index {
                0 | 1 => {
                    panic!();
                }
                2 => {}
                _ => (),
            }
        }

        if let Some(qdat) = &quarters[typedrawer.clicked_index] {
            quarterdrawer.buttons = qdat.iter().map(|f| f.name.clone()).collect();
            if let Some(cdat) = &courses[typedrawer.clicked_index][quarterdrawer.clicked_index] {
                let classnames = cdat
                    .iter()
                    .map(|f| f.get("CourseName").unwrap().as_str().unwrap().to_string());
                classdrawer.buttons = vec!["All".to_string()];
                classdrawer.buttons.extend(classnames);

                table.data = vec![];

                if typedrawer.clicked_index == 0 {
                    if classdrawer.clicked_index != 0 {
                        table.indecies = vec!["Date".into(), "Assignment".into(), "Grade".into()];

                        let class = &cdat[classdrawer.clicked_index - 1];
                        for i in class.get("Assignments").unwrap().as_array().unwrap() {
                            let mut row = vec![];

                            row.push(
                                i.get("AssignmentName")
                                    .unwrap()
                                    .as_str()
                                    .unwrap()
                                    .to_string(),
                            );

                            row.push(
                                i.get("AssignmentDate")
                                    .unwrap()
                                    .as_str()
                                    .unwrap()
                                    .to_string(),
                            );
                            row.push(
                                format!(
                                    "{}/{}",
                                    i.get("Score").unwrap().as_str().unwrap(),
                                    i.get("MaxPoints").unwrap().as_str().unwrap()
                                )
                                .to_string(),
                            );

                            table.data.push(row);
                        }
                    } else {
                        table.indecies = vec![
                            "Class".into(),
                            "Date".into(),
                            "Assignment".into(),
                            "Grade".into(),
                        ];
                        for course in cdat {
                            for i in course.get("Assignments").unwrap().as_array().unwrap() {
                                let mut row = vec![];
                                row.push(
                                    course
                                        .get("CourseName")
                                        .unwrap()
                                        .as_str()
                                        .unwrap()
                                        .to_string(),
                                );
                                let datestr = i
                                    .get("AssignmentDate")
                                    .unwrap()
                                    .as_str()
                                    .unwrap()
                                    .to_string();

                                row.push(parse_datestr(datestr));
                                row.push(
                                    i.get("AssignmentName")
                                        .unwrap()
                                        .as_str()
                                        .unwrap()
                                        .to_string(),
                                );

                                row.push(
                                    format!(
                                        "{}/{}",
                                        i.get("Score").unwrap().as_str().unwrap(),
                                        i.get("MaxPoints").unwrap().as_str().unwrap()
                                    )
                                    .to_string(),
                                );

                                table.data.push(row);
                            }
                        }
                    }
                } else if typedrawer.clicked_index == 1 {
                    classdrawer.buttons = vec![];
                    table.indecies = vec![
                        "Class".into(),
                        "Email".into(),
                        "Comments".into(),
                        "Grade".into(),
                    ];
                    for course in cdat {
                        let mut row = vec![];
                        row.push(
                            course
                                .get("CourseName")
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_string(),
                        );
                        row.push(
                            course.get("Faculty").unwrap().as_array().unwrap()[0]
                                .get("Email")
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_string(),
                        );
                        let Some(gradeobj) = course.get("TraditionalGrade") else { continue };
                        if gradeobj.is_null() {
                            continue;
                        }
                        row.push(
                            gradeobj
                                .get("Comments")
                                .unwrap()
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_str().unwrap().to_string())
                                .reduce(|s, acc| format!("{},{}", s, acc))
                                .unwrap_or_default(),
                        );
                        if let Some(grade) = gradeobj.get("Grade").unwrap().as_str() {
                            row.push(grade.to_string());
                        } else {
                            let avg = gradeobj
                                .get("GradeBookAverage")
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_string();
                            row.push(avg);
                        }

                        table.data.push(row);
                    }
                }
            } else {
                let (resp_tx, resp_rx) = oneshot::channel();
                tx.blocking_send(Command::QuarterData {
                    quarter: qdat[quarterdrawer.clicked_index].id.clone(),
                    data_type: data_from_index_scuffed_please_refactor(typedrawer.clicked_index),
                    resp: resp_tx,
                })
                .unwrap();
                courses[typedrawer.clicked_index][quarterdrawer.clicked_index] =
                    Some(t.poll_blocking(resp_rx).unwrap().courses);
            }
        } else {
            let (resp_tx, resp_rx) = oneshot::channel();
            tx.blocking_send(Command::QuarterData {
                quarter: Value::Null,
                data_type: data_from_index_scuffed_please_refactor(typedrawer.clicked_index),
                resp: resp_tx,
            })
            .unwrap();

            let data = t.poll_blocking(resp_rx).unwrap();

            let activequarter = data
                .quarters
                .iter()
                .position(|q| q.id == data.activequarter)
                .unwrap();
            courses[typedrawer.clicked_index][activequarter] = Some(data.courses);
            quarters[typedrawer.clicked_index] = Some(data.quarters);
            quarterdrawer.clicked_index = activequarter;
            quarterdrawer.index = activequarter;
        }

        if should_resize {
            // can be a self. later
            let top_h = 5;

            let mut current_y = 0;

            profdisplay.rect.h = top_h;
            profdisplay.rect.w = profdisplay.text.len() as u32 + 4;

            current_y += 5;

            if profdisplay.rect.w + 25 > t.size.cols as u32 {
                profdisplay.rect.w = t.size.cols as u32;

                topdrawer.rect.y = top_h;
                topdrawer.rect.h = top_h;
                topdrawer.rect.w = t.size.cols as u32;
                topdrawer.rect.x = 0;

                current_y += 5;
            } else {
                topdrawer.rect.y = 0;
                topdrawer.rect.x = profdisplay.rect.w;
                topdrawer.rect.h = top_h;
                topdrawer.rect.w = t.size.cols as u32 - profdisplay.rect.w;
            }
            typedrawer.rect.y = current_y;
            typedrawer.rect.x = 0;
            typedrawer.rect.w = t.size.cols as u32;
            typedrawer.rect.h = 3;
            current_y += 3;

            if profdisplay.rect.w + 25 > t.size.cols as u32 {
                quarterdrawer.rect.y = current_y;
                quarterdrawer.rect.w = t.size.cols as u32;
                current_y += 3;
                classdrawer.rect.y = current_y;
                classdrawer.rect.w = t.size.cols as u32;
                current_y += 3;
                classdrawer.rect.x = 0;
            } else {
                quarterdrawer.rect.y = current_y;
                quarterdrawer.rect.w = t.size.cols as u32 / 2;
                classdrawer.rect.y = current_y;
                classdrawer.rect.w = t.size.cols as u32 / 2;

                classdrawer.rect.x = t.size.cols as u32 / 2;
                current_y += 3;
            }
            quarterdrawer.rect.h = 3;
            classdrawer.rect.h = 3;
            quarterdrawer.rect.x = 0;

            table.rect.y = current_y;
            table.rect.x = 0;
            table.rect.w = t.size.cols as u32;
            table.rect.h = t.size.rows as u32 - current_y;
        }

        let mut vcs = vec![
            profdisplay.as_widget(),
            topdrawer.as_widget(),
            typedrawer.as_widget(),
            quarterdrawer.as_widget(),
            classdrawer.as_widget(),
            table.as_widget(),
        ];

        let ev = t.engine.poll();
        match ev {
            Event::Frame => {
                t.engine.clear_screen();
                t.engine.fill(pixel::pxl_bg(' ', theme.bg));

                for (i, w) in vcs.iter_mut().enumerate() {
                    let scr = w.draw(&theme, i == selected_widget);
                    t.engine
                        .print_screen(w.rect().x as i32, w.rect().y as i32, &scr);
                }

                t.engine.draw();
            }

            Event::Resize(x, y) => {
                t.engine.resize(x.into(), y.into());
                t.size = Size { rows: y, cols: x };
                should_resize = true;
            }
            Event::Mouse(e) => {
                for (i, w) in vcs.iter_mut().enumerate() {
                    let rect = w.rect().clone();
                    if e.column as u32 >= rect.x
                        && e.row as u32 >= rect.y
                        && e.column as u32 <= rect.x + rect.w
                        && e.row as u32 <= rect.y + rect.h
                    {
                        if selected_widget == i {
                            w.feed_event(event::Event::Mouse(e));
                        } else {
                            selected_widget = i;
                        }
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
            }) => return Err(Box::new(Exit {})),
            Event::Key(k) => {
                let k = vcs[selected_widget].feed_event(event::Event::Key(k));
                let Some(k) = k else{continue};
                let event::Event::Key(k) = k else {continue};
                match k {
                    KeyEvent {
                        code: KeyCode::Right,
                        modifiers: KeyModifiers::NONE,
                    }
                    | KeyEvent {
                        code: KeyCode::Down,
                        modifiers: KeyModifiers::NONE,
                    } => {
                        if selected_widget < vcs.len() - 1 {
                            selected_widget += 1;
                        }
                    }

                    KeyEvent {
                        code: KeyCode::Left,
                        modifiers: KeyModifiers::NONE,
                    }
                    | KeyEvent {
                        code: KeyCode::Up,
                        modifiers: KeyModifiers::NONE,
                    } => {
                        if selected_widget > 0 {
                            selected_widget -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
fn parse_datestr(date: String) -> String {
    let timestamp = &date[6..date.len() - 2];
    let d = UNIX_EPOCH + Duration::from_millis(timestamp.parse().unwrap());
    let dt = DateTime::<Utc>::from(d);
    dt.format("%m/%d/%y").to_string()
}

fn data_from_index_scuffed_please_refactor(clicked_index: usize) -> String {
    match clicked_index {
        0 => "Assignments",
        1 => "Grades",
        _ => todo!(),
    }
    .into()
}

async fn net_thread(mut rx: Receiver<Command>) {
    let (api, student) = 'collector: loop {
        if let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Login {
                    username,
                    password,
                    baseurl,
                    resp,
                } => {
                    let Ok(api) = SchoolTool::new(baseurl, username, password).await else {
                        let _ = resp.send(None).unwrap();
                        continue;
                    };
                    let Ok(student) = api.get_student(None).await else {
                        let _ = resp.send(None).unwrap();
                        continue;
                    };
                    let _ = resp.send(Some(student.clone())).unwrap();
                    break 'collector (api, student);
                }
                _ => panic!(),
            }
        }
    };
    loop {
        if let Some(cmd) = rx.recv().await {
            match cmd {
                Command::QuarterData {
                    data_type,
                    quarter,
                    resp,
                } => {
                    let dat = api
                        .quarter_data(data_type.clone(), student.guid.clone(), quarter)
                        .await
                        .unwrap(); //todo!
                                   //
                                   // dbg!(&dat);
                    let quarters = dat
                        .get("MarkingPeriods")
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| Quarter {
                            id: f.get("Id").unwrap().clone(),
                            name: f.get("Name").unwrap().as_str().unwrap().into(),
                        })
                        .collect();

                    let fieldname = if &data_type == "Assignments" {
                        "AssignmentCourses"
                    } else if data_type == "Grades" {
                        "GradeCourses"
                    } else {
                        todo!()
                    }
                    .to_string();
                    let _ = resp
                        .send(QuarterDataResponse {
                            quarters,
                            activequarter: dat.get("SelectedMarkingPeriod_ID").unwrap().clone(),
                            courses: dat.get(fieldname).unwrap().as_array().unwrap().to_vec(),
                        })
                        .unwrap();
                }
                _ => panic!(),
            }
        }
    }
}
