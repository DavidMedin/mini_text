use super::rect;
use wgpu::Device;
use wgpu_glyph::{Text,GlyphBrush,SectionGeometry, GlyphPositioner,SectionGlyph, ab_glyph::Font};
pub struct Cursor {
    pos : (usize,usize),
    pub rect : rect::Rect,

    font_size : f32,
    screen_size : (u32,u32)
}


impl Cursor {
    pub fn new(state : &super::State, pos : (usize,usize)) -> Self {
        let color = super::rgb(250, 148, 148);
        let screen_size = (state.size.width,state.size.height); // size of the screen
        let rect = rect::Rect::new(&state.device,screen_size, (1,1),(0,0), (0,0),color);
        let mut cursor = Cursor { pos, rect, font_size: state.font_scale, screen_size};


        let cursor_pos = cursor.calc_cursor_pos(&state.glyph_brush,  &state.text[cursor.pos.0])
        .expect("0,0 should be a valid cursor location, all the time.");

        // update the new cursor
        // cursor.rect.set_pos(&state.device, x, y) = ( as usize, cursor_pos.1 as usize);
        cursor.update_cursor(&state.device, &state.glyph_brush, &state.text);
        cursor
    }

    fn calc_cursor_pos(&self, glyph_brush : &GlyphBrush<()>, text : &String) -> Option<(u32,u32)> {
        let font = &glyph_brush.fonts()[0];
        let layout = wgpu_glyph::Layout::default_single_line();
        let wgpu_text = Text::new(text.as_str()).with_scale(self.font_size);
        // TODO: Memoization / Caching
        let sec_glyphs = layout.calculate_glyphs(&[font], &SectionGeometry { screen_position: (0.0,0.0), bounds: (self.screen_size.0 as f32,self.screen_size.1 as f32) } , &[wgpu_text]);
        
        let mut i = 0;
        let mut last_x = 0;
        for SectionGlyph{glyph,..} in &sec_glyphs {
            // Get the width of the glyph
            let bound = font.glyph_bounds(glyph);
            let width = bound.width().round() as u32;

            if i == self.pos.0 {
                return Some( (glyph.position.x.round() as u32,width))
            }
            
            last_x = glyph.position.x.round() as u32 + width;
            i += 1;
        }

        if i == self.pos.0 {
            // Only runs for the cursor position right after the last character.
            return Some( ( last_x, 8) );
        }
        
        None
    }

    pub fn update_screen_size(&mut self,device : &wgpu::Device, screen_size: (u32,u32)) {
        self.screen_size = screen_size;
        self.rect.update_rect(device, screen_size);
    }

    pub fn update_cursor(&mut self,device : &Device, glyph_brush : &GlyphBrush<()>, text : &Vec<String>) {
        // update cursor rectangle position.        
        let (x,w) = self.calc_cursor_pos(glyph_brush, &text[self.pos.1])
            .expect("You are bad at programming.");
                    // TODO: Add scrolling. This place will be affected a lot.
        self.rect.set_rect(device,x,(self.pos.1 as f32 * self.font_size) as u32, w,self.font_size as u32);
    }

    // Does not update the cursor's rectangle.
    pub fn move_cursor(&mut self,text : &Vec<String>, direction : super::CursorMovement) {
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

    pub fn insert_text(&mut self,text : &mut Vec<String>, character : char) {
        match character {
            '\r' => {
                // TODO: catch these out of bounds errors, report, and do nothing in the future.
                let string : String = text[self.pos.1].drain(self.pos.0..).collect();
                text.insert(self.pos.1+1, string);
                self.pos.1 += 1;
                self.pos.0 = 0;
            },
            '\u{8}' => { // backspace
                if self.pos.0 > 0 {
                    text[self.pos.1].remove(if self.pos.0 == 0 {self.pos.0} else {self.pos.0-1} );
                    self.pos.0 -= 1;
                } else if self.pos.1 > 0 {
                    // Copy the remaining text from this line and copy to the last line.
                    let string : String = text[self.pos.1].drain(..).collect();
                    let len = text[self.pos.1-1].len();
                    text[self.pos.1-1].insert_str( len, string.as_str());
                    self.pos.0 = len;

                    text.remove(self.pos.1);
                    self.pos.1 -= 1;
                }
            }
            '\t' => {

            }
            character if character.is_control() == false => {
                text[self.pos.1].insert(self.pos.0,character);
                // text.last_mut().unwrap().push(character);
                self.pos.0 += 1;
            }, // unwrap should be safe.
            _ => {}
        }
        // self.update_cursor();
    }
}
