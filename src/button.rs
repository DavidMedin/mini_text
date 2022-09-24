use super::{rgb};
use super::rect::{Rect};
use wgpu::{Device};
pub enum BtnContent {
    Image(String),
    Text(String),
    None
}

pub struct ButtonBuilder {
    screen_size : (u32,u32),
    size : (u32,u32),
    pos : (i64,i64),
    color : (f32,f32,f32),
    content : BtnContent
}
impl ButtonBuilder {
    pub fn new(screen_size : (u32,u32)) -> Self {
        ButtonBuilder { screen_size, size: (100,75), pos: (0,0), color: rgb(44, 54, 57), content: BtnContent::None}
    }
    pub fn size(mut self, size : (u32,u32)) -> Self {
        self.size = size;
        self
    }
    pub fn pos(mut self,pos : (i64,i64)) -> Self {
        self.pos = pos;
        self
    }
    pub fn color(mut self, color : (f32,f32,f32)) -> Self {
        self.color = color;
        self
    }
    pub fn content(mut self, content : BtnContent) -> Self {
        self.content = content;
        self
    }

    pub fn build(self, device : &Device) -> Button {
        let rect = Rect::new(device,self.screen_size,self.size,self.pos,(0,0),self.color);
        Button {rect, content : self.content }
    }
}

pub struct Button {
    pub rect : Rect,
    content : BtnContent 
}

impl Button {
    pub fn does_click(&self, click_pos : (u32,u32) ) -> bool {
        if self.rect.px_pos.0 <= click_pos.0 as i64 &&
        self.rect.px_pos.0 + self.rect.px_size.0 as i64 >= click_pos.0 as i64 &&
        self.rect.px_pos.1 <= click_pos.1 as i64 &&
        self.rect.px_pos.1 + self.rect.px_size.1 as i64 >= click_pos.1 as i64 {
            return true;
        }
        false
    }

    // I don't understand lifetimes
    pub fn draw<'a>(&'a self,render_pass : &mut wgpu::RenderPass<'a>) {
        self.rect.draw(render_pass);
    }

    pub fn update(&mut self, device : &wgpu::Device, screen_size:(u32,u32)) {
        self.rect.update_rect(device, screen_size);
    }
}