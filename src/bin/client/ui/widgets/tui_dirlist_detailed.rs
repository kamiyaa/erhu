use std::cmp::Ordering;

use dizi::song::DiziAudioFile;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::config::option::DisplayOption;
use crate::fs::{FileType, JoshutoDirEntry, JoshutoDirList, LinkType};
use crate::util::format;
use crate::util::string::UnicodeTruncate;
use crate::util::style;

const MIN_LEFT_LABEL_WIDTH: i32 = 15;

const ELLIPSIS: &str = "…";

pub struct TuiDirListDetailed<'a> {
    dirlist: &'a JoshutoDirList,
    display_options: &'a DisplayOption,
    currently_playing: Option<&'a DiziAudioFile>,
    focused: bool,
}
impl<'a> TuiDirListDetailed<'a> {
    pub fn new(
        dirlist: &'a JoshutoDirList,
        display_options: &'a DisplayOption,
        currently_playing: Option<&'a DiziAudioFile>,
        focused: bool,
    ) -> Self {
        Self {
            dirlist,
            display_options,
            currently_playing,
            focused,
        }
    }

    fn draw_listing(&self, area: &Rect, buf: &mut Buffer) {
        let x = area.left();
        let y = area.top();
        let curr_index = match self.dirlist.get_index() {
            Some(i) => i,
            None => {
                let style = Style::default().bg(Color::Red).fg(Color::White);
                buf.set_stringn(x, y, "empty", area.width as usize, style);
                return;
            }
        };

        let drawing_width = area.width as usize;
        let skip_dist = self.dirlist.first_index_for_viewport();

        let space_fill = " ".repeat(drawing_width);

        // draw every entry
        self.dirlist
            .iter()
            .skip(skip_dist)
            .enumerate()
            .take(area.height as usize)
            .for_each(|(i, entry)| {
                let ix = skip_dist + i;

                let style = if self.focused && ix == curr_index {
                    style::entry_style(entry).add_modifier(Modifier::REVERSED)
                } else if let Some(song) = self.currently_playing {
                    if song.file_path() == entry.file_path() {
                        style::playing_style()
                    } else {
                        style::entry_style(entry)
                    }
                } else {
                    style::entry_style(entry)
                };

                buf.set_string(x, y + i as u16, space_fill.as_str(), style);

                let line_number_string = "".to_string();
                print_entry(
                    buf,
                    entry,
                    style,
                    (x + 1, y + i as u16),
                    drawing_width - 1,
                    line_number_string,
                );
            });
    }
}

impl<'a> Widget for TuiDirListDetailed<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 1 {
            return;
        }
        self.draw_listing(&area, buf);
    }
}

fn print_entry(
    buf: &mut Buffer,
    entry: &JoshutoDirEntry,
    style: Style,
    (x, y): (u16, u16),
    drawing_width: usize,
    index: String,
) {
    let size_string = match entry.metadata.file_type() {
        FileType::Directory => entry
            .metadata
            .directory_size()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "".to_string()),
        FileType::File => format::file_size_to_string(entry.metadata.len()),
    };
    let symlink_string = match entry.metadata.link_type() {
        LinkType::Normal => "",
        LinkType::Symlink(_, _) => "-> ",
    };
    let left_label_original = entry.file_name();
    let right_label_original = format!(" {}{} ", symlink_string, size_string);

    let (left_label, right_label) = factor_labels_for_entry(
        left_label_original,
        right_label_original.as_str(),
        drawing_width,
    );

    let index_width = index.width();
    // draw_index
    buf.set_stringn(x, y, index, index_width, Style::default());

    let drawing_width = drawing_width - index_width as usize;
    let x = x + index_width as u16;
    // Drawing labels
    buf.set_stringn(x, y, left_label, drawing_width, style);
    buf.set_stringn(
        x + drawing_width as u16 - right_label.width() as u16,
        y,
        right_label,
        drawing_width,
        style,
    );
}

fn factor_labels_for_entry<'a>(
    left_label_original: &'a str,
    right_label_original: &'a str,
    drawing_width: usize,
) -> (String, &'a str) {
    let left_label_original_width = left_label_original.width();
    let right_label_original_width = right_label_original.width();

    let left_width_remainder = drawing_width as i32 - right_label_original_width as i32;
    let width_remainder = left_width_remainder as i32 - left_label_original_width as i32;

    if drawing_width == 0 {
        ("".to_string(), "")
    } else if width_remainder >= 0 {
        (left_label_original.to_string(), right_label_original)
    } else if left_width_remainder < MIN_LEFT_LABEL_WIDTH {
        (
            if left_label_original.width() as i32 <= left_width_remainder {
                trim_file_label(left_label_original, drawing_width)
            } else {
                left_label_original.to_string()
            },
            "",
        )
    } else {
        (
            trim_file_label(left_label_original, left_width_remainder as usize),
            right_label_original,
        )
    }
}

pub fn trim_file_label(name: &str, drawing_width: usize) -> String {
    // pre-condition: string name is longer than width
    let (stem, extension) = match name.rfind('.') {
        None => (name, ""),
        Some(i) => name.split_at(i),
    };
    if drawing_width < 1 {
        "".to_string()
    } else if stem.is_empty() || extension.is_empty() {
        let full = format!("{}{}", stem, extension);
        let mut truncated = full.trunc(drawing_width - 1);
        truncated.push_str(ELLIPSIS);
        truncated
    } else {
        let ext_width = extension.width();
        match ext_width.cmp(&drawing_width) {
            Ordering::Less => {
                let stem_width = drawing_width - ext_width;
                let truncated_stem = stem.trunc(stem_width - 1);
                format!("{}{}{}", truncated_stem, ELLIPSIS, extension)
            }
            Ordering::Equal => extension.replacen('.', ELLIPSIS, 1),
            Ordering::Greater => {
                // file ext does not fit
                let stem_width = drawing_width;
                let truncated_stem = stem.trunc(stem_width - 3);
                format!("{}{}.{}", truncated_stem, ELLIPSIS, ELLIPSIS)
            }
        }
    }
}
