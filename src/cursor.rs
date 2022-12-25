use crate::State;

use super::rect;
use super::Line;
use wgpu::Device;
use wgpu_glyph::{Text,GlyphBrush,SectionGeometry, GlyphPositioner,SectionGlyph, ab_glyph::{Font, FontArc}};
pub struct Cursor {
    pos : (usize,usize),
    pub rect : rect::Rect,

    font_size : f32,
    screen_size : (u32,u32)
}


impl Cursor {
    pub fn new(state : &super::State, pos : (usize,usize)) -> Self {
        let color = super::rgb(super::margin_bg_color);
        let screen_size = (state.size.width,state.size.height); // size of the screen
        let rect = rect::Rect::new(&state.device,screen_size, (1,1),(0,0), (0,0),color);
        let mut cursor = Cursor { pos, rect, font_size: state.font_scale, screen_size};


        let cursor_pos = cursor.calc_cursor_pos(&state.glyph_brush,  &state.lines[pos.1])
        .expect("0,0 should be a valid cursor location, all the time.");

        // update the new cursor
        // cursor.rect.set_pos(&state.device, x, y) = ( as usize, cursor_pos.1 as usize);
        cursor.update_cursor(&state.device, &state.glyph_brush,state.scroll as i64 * state.font_scale as i64, &state.lines);
        cursor
    }

    fn calc_cursor_pos(&self, glyph_brush : &GlyphBrush<()>, text : &Line) -> Option<(i64,i64,u32)> {
        let font = &glyph_brush.fonts()[0];

        // let mut text_acc = 0;
        let mut x = 0;
        let mut y = 0;
        let mut last_x_px : i64 = 0;
        for line in &text.glyphs {
            last_x_px = 0;
            for SectionGlyph{glyph,..} in line {
                // Get the width of the glyph
                let bound = font.glyph_bounds(glyph);
                let width = bound.width().round() as u32;
    
                // This glyph is what the cursor is highlighting. Return its position.
                if x == self.pos.0 {
                    return Some( (glyph.position.x.round() as i64, (y as f32 * self.font_size) as i64,width))
                }
                
                last_x_px = (glyph.position.x.round() as u32 + width) as i64;
                x += 1;
            }

            // text_acc += line.len();
            y += 1;
        }
        y-=1; // to get rid of the last y change, so the cursor can hang off the side of the line.

        if x == self.pos.0 {
            // Only runs for the cursor position right after the last character.
            return Some( ( last_x_px, (y as f32 * self.font_size) as i64, 8) );
        }
        
        None
    }

    pub fn update_screen_size(&mut self,device : &wgpu::Device, screen_size: (u32,u32)) {
        self.screen_size = screen_size;
        self.rect.update_rect(device, screen_size);
    }

    // scaled_scroll is a scroll of pixels.
    pub fn update_cursor(&mut self,device : &Device , glyph_brush : &GlyphBrush<()>,scaled_scroll : i64, text : &Vec<Line>) {
        // get number of lines proceeding.
        let mut y_acc = 0;
        for i in 0..self.pos.1 { // doesn't include self.pos.1
            y_acc += text[i].glyphs.len() as i64;
        }
        // update cursor rectangle position.        
        let (x,y,w) = self.calc_cursor_pos(glyph_brush, &text[self.pos.1])
            .expect("You are bad at programming.");
        self.rect.set_rect(device,self.screen_size,x,y + y_acc * (self.font_size as i64) - scaled_scroll, w,self.font_size as u32);
    }

    // Does not update the cursor's rectangle.
    pub fn move_cursor(&mut self,text : &Vec<&String>, direction : super::CursorMovement) {
        // TODO: Add ghost cursor to try to keep horizontal position. This is opinionated!
        use super::CursorMovement::*;
        match direction {
            Left => {
                if self.pos.0 > 0 {
                    self.pos.0 -= 1;
                }else if self.pos.1 > 0{
                    // move to line above
                    self.pos.1 -= 1;
                    self.pos.0 = text[self.pos.1].len();
                }
            }
            Right => {
                // if there is room on this line
                if self.pos.0 < text[self.pos.1].len() {
                    self.pos.0 += 1;

                // If there is another line below to move too.
                }else if text.len()-1 > self.pos.1{
                    // move to line below
                    self.pos.0 = 0;
                    self.pos.1 += 1;
                }
            }
            Down => {
                if text.len()-1 > self.pos.1 {
                    self.pos.1 += 1;
                    // move .0 to the correct place.
                    if self.pos.0 > text[self.pos.1].len() {
                        self.pos.0 = text[self.pos.1].len();
                    }
                }
            }
            Up => {
                if self.pos.1 > 0 {
                    self.pos.1 -= 1;
                    //move .0
                    if self.pos.0 > text[self.pos.1].len() {
                        self.pos.0 = text[self.pos.1].len();
                    }
                }
            }
            _ => {}
        }   
    }

    // Given a line (a string and a list of breakers), return the slice of the string that the cursor is in.
    pub fn get_cursor_inline<'a>(&self, text : &'a(String,Vec<usize>)) -> Option<&'a str> {
        let mut acc = 0;
        for i in 0..text.1.len() {
            if self.pos.0 < text.1[i] {
                return Some( &text.0[text.1[i-1]..text.1[i]]);
            }
        }
        None
    }
    pub fn insert_text(&mut self,glyph_brush : &GlyphBrush<()>, lines : &mut Vec<Line>, character : char) {
        let Line { text, breaks, glyphs } = &mut lines[self.pos.1];
        match character {
            '\r' => {
                if self.pos.0 > text.len() {
                    panic!("Cursor is too far into a line!");
                }
                
                let string : String = text.drain(self.pos.0..).collect();
                // update the 'drained' string.
                *glyphs = State::batch_read_string(glyph_brush, self.font_size, self.screen_size, text);
                *breaks = State::wrap_line(glyphs,text); // update line break indices.

                // calculate the new line's line breaks.
                let line_glyphs = State::batch_read_string(glyph_brush, self.font_size, self.screen_size, &string);
                let new_breaks = State::wrap_line(&line_glyphs,&string);
                let new_line = Line{ text: string, breaks : new_breaks, glyphs: line_glyphs };
                lines.insert(self.pos.1+1, new_line);
                self.pos.1 += 1;
                self.pos.0 = 0;
            },
            '\u{8}' => { // backspace
                let mut update_line = self.pos.1;
                if self.pos.0 > 0 {
                    text.remove(if self.pos.0 == 0 {self.pos.0} else {self.pos.0-1} );
                    self.pos.0 -= 1;
                } else if self.pos.1 > 0 {
                    // Copy the remaining text from this line and copy to the last line.
                    let Line{ text, .. } = lines.remove(self.pos.1);
                    let len = lines[self.pos.1-1].text.len();

                    lines[self.pos.1-1].text.insert_str( len, text.as_str());
                    self.pos.0 = len;
                    
                    self.pos.1 -= 1;
                    
                    update_line = self.pos.1;
                }
                // update
                let line_glyphs = State::batch_read_string(glyph_brush, self.font_size, self.screen_size, &lines[update_line].text);
                lines[update_line].breaks = State::wrap_line(&line_glyphs,&lines[update_line].text); // update line break indices.
                lines[update_line].glyphs = line_glyphs;
            }
            '\t' => {

            }
            character if character.is_control() == false => {
                text.insert(self.pos.0,character);
                // TODO: be smarter, don't totally recalcuate everything all the time.
                let line_glyphs = State::batch_read_string(glyph_brush, self.font_size, self.screen_size, text);
                *breaks = State::wrap_line(&line_glyphs,text); // update line break indices.
                *glyphs = line_glyphs;
                
                self.pos.0 += 1;
            }, // unwrap should be safe.
            _ => {}
        }
    }
}
