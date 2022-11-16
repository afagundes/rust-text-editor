extern crate libc;

use std::char;
use std::env;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::process;

use libc::termios;

const SYSTEM_OUT_FD: libc::c_int = 0;

const ARROW_UP: u16 = 1000;
const ARROW_DOWN: u16 = 1001;
const ARROW_RIGHT: u16 = 1002;
const ARROW_LEFT: u16 = 1003;
const HOME: u16 = 1004;
const END: u16 = 1005;
const DEL: u16 = 1006;
const PAGE_UP: u16 = 1007;
const PAGE_DOWN: u16 = 1008;

struct Editor {
    cursor_x: usize,
    cursor_y: usize,
    columns: usize,
    rows: usize,
    offset_y: usize,
    original_terminal_props: Option<termios>,
    content: Vec<String>,
    filename: String,
}

fn main() {
    let mut editor = Editor {
        cursor_x: 0,
        cursor_y: 0,
        columns: 0,
        rows: 0,
        offset_y: 0,
        original_terminal_props: None,
        content: Vec::new(),
        filename: String::new(),
    };

    open_editor(&mut editor);
    enable_raw_mode(&mut editor);
    set_window_size(&mut editor);

    loop {
        scroll(&mut editor);
        refresh_screen(&editor);
        let last_char = read_key();
        handle_key(last_char, &mut editor);
    }
}

fn open_editor(editor: &mut Editor) {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        let file_path = &args[1];
        let file = File::open(file_path);

        match file {
            Ok(f) => {
                for line in BufReader::new(f).lines() {
                    if let Ok(l) = line {
                        editor.content.push(l);
                    }
                }

                editor.filename = String::from(extract_filename(file_path));
            }
            Err(_err) => {
                // TODO print message at status bar
            }
        }
    }
}

fn extract_filename(file_path: &str) -> &str {
    let mut last_slash_index = 0;

    for (i, &char) in file_path.as_bytes().iter().enumerate() {
        if char == b'/' {
            last_slash_index = i + 1;
        }
    }

    &file_path[last_slash_index..]
}

fn enable_raw_mode(editor: &mut Editor) {
    unsafe {
        let mut termios = termios {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        };
        let rc: libc::c_int = libc::tcgetattr(SYSTEM_OUT_FD, &mut termios);

        if rc != 0 {
            eprintln!("There was a problem calling tcgetattr");
            process::exit(rc);
        }

        let original_attributes = termios.clone();

        termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
        termios.c_iflag &= !(libc::IXON | libc::ICRNL);
        termios.c_oflag &= !(libc::OPOST);

        termios.c_cc[libc::VMIN] = 0;
        termios.c_cc[libc::VTIME] = 1;

        libc::tcsetattr(SYSTEM_OUT_FD, libc::TCSAFLUSH, &mut termios);

        editor.original_terminal_props = Some(original_attributes);
    }
}

fn set_window_size(editor: &mut Editor) {
    let (columns, rows) = term_size::dimensions().expect("Unable to get terminal size");
    editor.columns = columns;
    editor.rows = rows - 1;
}

fn scroll(editor: &mut Editor) {
    if editor.cursor_y >= editor.rows + editor.offset_y {
        editor.offset_y = editor.cursor_y - editor.rows + 1;
    } else if editor.cursor_y < editor.offset_y {
        editor.offset_y = editor.cursor_y;
    }
}

fn refresh_screen(editor: &Editor) {
    let mut builder = String::new();

    move_cursor_to_top_left(&mut builder);
    draw_content(editor, &mut builder);
    draw_status_bar(editor, &mut builder);
    draw_cursor(editor, &mut builder);

    write(builder.as_bytes());
}

fn draw_status_bar(editor: &Editor, builder: &mut String) {
    let mut status_message = String::from(" Ari Code's Editor - v0.0.1 - Rust Edition - ");
    status_message.push_str(get_file_name(editor));

    let mut info_message = String::from("Line: ");
    info_message.push_str(editor.cursor_y.to_string().as_str());
    info_message.push(' ');

    builder.push_str("\x1b[7m"); // reverse background and foreground colors
    builder.push_str(status_message.as_str());
    builder.push_str(
        " ".repeat(editor.columns - status_message.len() - info_message.len())
            .as_str(),
    );
    builder.push_str(info_message.as_str());
    builder.push_str("\x1b[0m");
}

fn get_file_name(editor: &Editor) -> &str {
    if editor.filename.len() == 0 {
        "New File"
    } else {
        &editor.filename
    }
}

fn draw_content(editor: &Editor, builder: &mut String) {
    for i in 0..editor.rows {
        let file_i = editor.offset_y + i;

        if file_i >= editor.content.len() {
            builder.push_str("~");
        } else {
            builder.push_str(editor.content[file_i].as_str());
        }

        builder.push_str("\x1b[K\r\n");
    }
}

fn move_cursor_to_top_left(builder: &mut String) {
    //builder.push_str("\x1b[2J"); // clear the screen
    builder.push_str("\x1b[H"); // set cursor at 0,0
}

fn draw_cursor(editor: &Editor, builder: &mut String) {
    builder.push_str(
        format!(
            "\x1b[{};{}H",
            editor.cursor_y - editor.offset_y + 1,
            editor.cursor_x + 1
        )
        .as_str(),
    ); // set cursor position
}

fn read_key() -> u16 {
    let key = read();
    if key != '\x1b' {
        return key as u16;
    }

    let next_key = read();
    if next_key != '[' && next_key != 'O' {
        return next_key as u16;
    }

    let yet_another_key = read();
    return if next_key == '[' {
        match yet_another_key {
            'A' => ARROW_UP,
            'B' => ARROW_DOWN,
            'C' => ARROW_RIGHT,
            'D' => ARROW_LEFT,
            'H' => HOME,
            'F' => END,
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                let yet_another_char = read();
                if yet_another_char != '~' {
                    return yet_another_char as u16;
                }

                return match yet_another_key {
                    '1' | '7' => HOME,
                    '3' => DEL,
                    '4' | '8' => END,
                    '5' => PAGE_UP,
                    '6' => PAGE_DOWN,
                    _ => yet_another_key as u16,
                };
            }
            _ => yet_another_key as u16,
        }
    } else {
        // nextKey == O
        match yet_another_key {
            'H' => HOME,
            'F' => END,
            _ => yet_another_key as u16,
        }
    };
}

fn handle_key(key: u16, editor: &mut Editor) {
    let key_char = char::from_u32(key as u32).unwrap();

    if key_char as char == 'q' {
        exit(&mut editor.original_terminal_props.unwrap());
    } else if vec![ARROW_UP, ARROW_DOWN, ARROW_LEFT, ARROW_RIGHT, HOME, END].contains(&key) {
        move_cursor(key, editor);
    }
}

fn exit(termios: &mut termios) {
    write("\x1b[2J".as_bytes()); // clear screen
    write("\x1b[H".as_bytes()); // set cursor at 0,0

    unsafe {
        libc::tcsetattr(SYSTEM_OUT_FD, libc::TCSAFLUSH, termios);
    }

    process::exit(0);
}

fn move_cursor(key: u16, editor: &mut Editor) {
    match key {
        ARROW_UP => {
            if editor.cursor_y > 0 {
                editor.cursor_y -= 1;
            }
        }
        ARROW_DOWN => {
            if editor.cursor_y < editor.content.len() {
                editor.cursor_y += 1;
            }
        }
        ARROW_LEFT => {
            if editor.cursor_x > 0 {
                editor.cursor_x -= 1;
            }
        }
        ARROW_RIGHT => {
            if editor.cursor_x < editor.columns - 1 {
                editor.cursor_x += 1;
            }
        }
        HOME => editor.cursor_x = 0,
        END => editor.cursor_x = editor.columns - 1,
        _ => {}
    };
}

fn read() -> char {
    let mut buffer = [0; 1];

    while buffer[0] == 0 {
        io::stdin()
            .read(&mut buffer)
            .expect("Error reading user input");
    }

    buffer[0] as char
}

fn write(buffer: &[u8]) {
    let mut stdout = io::stdout().lock();

    stdout
        .write(buffer)
        .expect("Error writing to output stream");

    stdout.flush().expect("Error flushing buffer");
}
