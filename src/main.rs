mod rect;
mod cursor;
mod button;

/*
TODO: subscribe to file updates
TODO: line numbers
TODO: draw play area and margin separetly, and blit together
TODO: fun timing and color things. like a fading cursor.
TODO: Copy and Paste
TODO: Alt - drag for windows (shift for window resize)
TODO: dbus magic
 */

use std::{io::{BufRead, Write}};
use cursor::Cursor;
use wgpu::{util::StagingBelt, Device, RenderPass};
use wgpu_glyph::{*,ab_glyph::{self, Font, FontArc}, GlyphBrushBuilder, GlyphBrush, Section, Text, GlyphPositioner, SectionGeometry};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder,Window}, dpi::PhysicalPosition,
};

// Don't use this will textures! Probably not a problem, but textures are
//  stored in the sRGB format. This function is rgb -> sRGB
fn rgb(r:u32,g:u32,b:u32) -> (f32,f32,f32) {
    // approximated color correction formula
    // (rgb_color / 255) ^ 2.2
    ((r as f32/255.0).powf(2.2),(g as f32/255.0).powf(2.2),(b as f32/255.0).powf(2.2))
}


pub struct Line {
    text : String,
    breaks : Vec<usize>,
    glyphs : Vec<Vec<SectionGlyph>>
}
impl Line {
    pub fn new(text : String, glyph_brush : &GlyphBrush<()>, font_scale : f32, screen_size : (u32,u32) ) -> Self {
        let glyphs = State::batch_read_string(&glyph_brush, font_scale, screen_size, &text);
        let breaks = State::wrap_line(&glyphs, &text);
        Line { text, breaks, glyphs}
    }
    pub fn calculate(&mut self, glyph_brush : &GlyphBrush<()>, font_scale : f32, screen_size : (u32,u32) ) {
        self.glyphs = State::batch_read_string(&glyph_brush, font_scale, screen_size, &self.text);
        self.breaks = State::wrap_line(&self.glyphs, &self.text);
    }
}

// TODO: Center file_name text
struct TopMargin {
    rect : rect::Rect,
    left_icon : rect::Rect,
    file_name : String,
    exit_button : button::Button
}
impl TopMargin {
    pub fn new(device : &Device, screen_size : (u32,u32), file_name :String) -> Self {
        let margin_height = 20;
        let margin_rect = rect::Rect::new(device,screen_size,(screen_size.0,margin_height), (0,0), (0,0), rgb(120, 149, 178));
        let left_icon = rect::Rect::new(device, screen_size,(16,16), (2,2), (0,0), rgb(195, 255, 153));

        let exit_button = button::ButtonBuilder::new(screen_size).size((16,16)).pos((screen_size.0 as i64-20, 2))
            .color(rgb(246, 90, 131)).build(device);
        
        TopMargin { rect: margin_rect, left_icon, file_name: file_name,exit_button }
    }

    pub fn draw<'a>(&'a mut self,device : &wgpu::Device, render_pass : & mut wgpu::RenderPass<'a>, modified : bool) {
        let color : (f32,f32,f32) = if modified == true { rgb(246, 90, 131) } else { rgb(195, 255, 153) };
        self.left_icon.set_color(device, color);
        self.rect.draw(render_pass);
        self.left_icon.draw(render_pass);
        self.exit_button.draw(render_pass);
    }

}

// The graphical state of the window.
pub struct State {
    surface : wgpu::Surface,
    device : wgpu::Device,
    queue : wgpu::Queue,
    config : wgpu::SurfaceConfiguration,
    size : winit::dpi::PhysicalSize<u32>,
    glyph_brush : GlyphBrush<()>,
    staging_belt : StagingBelt,

    rect_pipeline : rect::RectPipeline,

    file_name : String,
    lines : Vec<Line>,
    font_scale : f32,

    cursors : Vec<Cursor>,
    rectangles: Vec<rect::Rect>,

    top_margin : TopMargin,
    modified : bool, // Has the document been modified?

    scroll : f64
}

#[derive(Clone,Copy)]
pub enum CursorMovement {
    Up,Down,Left,Right
}
// colors: https://colorhunt.co/palette/100f0f0f3d3ee2dcc8f1f1f1

impl State {
    async fn new(window: &Window,file_name : String) -> Self{
        let size = window.inner_size();

        // Instance is a handle to the gpu or whatever is computing gfx.
        // only used to create surfaces and adapters.
        // backends::all is vulkan metal dx12 or browswer stuff
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };

        // actual handle to the gpu.
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions{
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }
        ).await.unwrap();

        let (device,queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(), // what features do we want?
                limits: wgpu::Limits::default(),
            }
            , None).await.unwrap();

            // will need to be regenerated for every resize of window.
            let config = wgpu::SurfaceConfiguration{
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface.get_supported_formats(&adapter)[0], // Maybe CRT problem.
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::Fifo,
            };
            surface.configure(&device, &config);

            // TODO: Search paths for user specified font. Or use user's specified path.
            // /home/david/.local/share/fonts/Vulf_Mono-Light_Italic_web.ttf
            // ../Monocraft.otf
            let vulf = ab_glyph::FontArc::try_from_slice(include_bytes!("../Monocraft.otf")).unwrap();
            let glyph_brush = GlyphBrushBuilder::using_font(vulf).build(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
            let staging_belt = wgpu::util::StagingBelt::new(1024);
            let font_scale = 16.0;

            let file_lines : Vec<String> = {// open the file ---------------------------------|
                let path = std::path::Path::new(&file_name);
                
                match std::fs::OpenOptions::new().read(true).open(path){
                    Ok(file) => {
                        let mut text : Vec<String> = vec![];
                        let reader = std::io::BufReader::new(file);
                        for line in reader.lines() { // from_utf8_lossy for binary files. Want differnt mode!
                            // if the file contains bad text, dump the text so far, report error, and break.
                            let line = if let Ok(line) = line {line} else {text = vec![]; println!("Failed to read file : contains invalid utf-8!"); break;};
                            
                            // copy the file into text.
                            text.push(line);
                        }

                        if text.len() == 0 { // If the file is emtpy, add an empty line
                            text.push(String::new())
                        }
                        
                        // close the file by dropping the File object.
                        text // Return to file_lines <---------------------------------------------|
                    },
                    Err(e ) => {
                        // todo!();//TODO: Handle some of the errors from e
                        if let std::io::ErrorKind::NotFound = e.kind() {
                            vec![String::new()]
                        } else {
                            panic!("Error opening file! : {}",e);
                        }
                    },
                }
                
            };

            let mut lines : Vec<Line> = vec![];
            for line in file_lines {
                lines.push( Line::new(line,&glyph_brush, font_scale, (size.width,size.height)) );
            }

            let rect_pipeline = rect::RectPipeline::new(&device, config.format);

            // let cursor : (usize,usize) = (0,0);
            let rectangles = vec![];
            // create a bunch of rectangles

            let top_margin = TopMargin::new(&device, (size.width,size.height), file_name.clone());
            

            let mut state = Self { surface, device, queue, config, size, glyph_brush, staging_belt, rect_pipeline, rectangles, font_scale,file_name,cursors : vec![], scroll : 0.0 , lines, top_margin, modified:false };

            let cursor : cursor::Cursor = cursor::Cursor::new(&state, (0,0));
            state.cursors.push(cursor);

            state
    }

    fn wrap_line( glyphs : &Vec<Vec<SectionGlyph>>, text : &String) -> Vec<usize> {
        // a list of pairs of a string and its breaks. Each break is a seperator of lines from word wrap.
        let mut acc = 0;
        let mut breaks : Vec<usize> = glyphs.iter().map(|x| {
            acc += x.len();
            acc
        }).collect();
        breaks.insert(0,0); // makes life easier
        breaks
    }
    
    // Read a big string, and generate the needed sections
    fn batch_read_string(glyph_brush : &GlyphBrush<()>,font_size : f32, screen_size : (u32,u32), text : &String) -> Vec<Vec<SectionGlyph>> {
        // TODO: Custom layout that supports single-word character wrapping. Pain awaits.
        let font = &glyph_brush.fonts()[0]; // TODO: Font managing (Low priority)
        let layout = wgpu_glyph::Layout::default_single_line();
        
        let mut wgpu_texts = vec![ Text::new(text.as_str()).with_scale(font_size) ];
                                                                                            // -8 to give room for the cursor.
        let sec_geom = SectionGeometry { screen_position: (0.0,0.0), bounds: (screen_size.0 as f32 - 8.0,screen_size.1 as f32) };
        let mut sec_glyphs = layout.calculate_glyphs(&[font], &sec_geom , wgpu_texts.as_slice());

        let mut finished_glyphs : Vec<Vec<SectionGlyph>> = vec![];
        
        let mut i = 0;
        let mut acc_length = 0;

        while acc_length + sec_glyphs.len() != text.len() {
            // --------------- Archive this text. It is the right length ----------------
            //                                                               v--- because it is the right operand of '..', it is exclusive.
            let slice = acc_length..acc_length+sec_glyphs.len();
            wgpu_texts[i] = Text::new(&text[slice]).with_scale(font_size);
            finished_glyphs.push( sec_glyphs);
            acc_length += wgpu_texts[i].text.len();

            let potential_text = Text::new(&text[acc_length..]).with_scale(font_size);
            sec_glyphs = layout.calculate_glyphs(&[font], &sec_geom, &[potential_text]);
            wgpu_texts.push( potential_text );

            i += 1;
        }

        // compute last string to glyph
        finished_glyphs.push(sec_glyphs);
        return finished_glyphs;
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
		if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.size = new_size;

            // reconfigure the device.
            self.surface.configure(&self.device, &self.config);

            // recalculate word wrap
            for line in &mut self.lines {
                line.calculate(&self.glyph_brush, self.font_scale, (self.size.width,self.size.height));
            }

            // recalculate rectangles
            // self.cursor.update_rect(&self.device,(new_size.width,new_size.height));
            for cursor in &mut self.cursors {
                cursor.update_screen_size(&self.device, (new_size.width,new_size.height));
            }

            for rect in &mut self.rectangles{
                rect.update_rect(&self.device, (new_size.width,new_size.height));
            }

            self.top_margin.rect.update_rect(&self.device, (new_size.width,new_size.height));
        }
	}


    fn save_file(&mut self) {
        // Attempt to open file.
        let path = std::path::Path::new(&self.file_name);
        println!("Opening {:?}",path);
        //  Like Open("file", 'w') in C, I think.
        let mut file = match std::fs::OpenOptions::new().write(true).truncate(true).create(true).open(path) {
            // let mut file = match std::fs::OpenOptions::new().truncate(true).open(path) {
            Ok(file) => file,
            Err(e) => {
                // TODO: don't panic, but tell the user failed to open file, graphically.
                println!("{:?}", e);

                todo!();
            },
        };

        // erase file, and write to it.
        let mut i : usize = 0;
        let len = self.lines.len();
        for Line{text,..} in &self.lines {
            // TODO: handle these
            file.write(text.as_bytes());
            i+=1;
            if i != len {
                file.write(&['\n' as u8]);
            }
        }
        self.modified = false;
    }
	fn input(&mut self, event : &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {

    }

    fn move_cursor(&mut self, direction : CursorMovement) {
        for cursor in &mut self.cursors {
            let refs : Vec<&String> = self.lines.iter().map(|x| &x.text).collect();
            cursor.move_cursor(&refs, direction);
            cursor.update_cursor(&self.device, &self.glyph_brush, self.scroll as i64 * self.font_scale as i64, &self.lines);
        }
        
    }
    fn insert_cursor(&mut self, character : char) {
        // the cursor is an index. backspace removes the character before the cursor.
        self.modified = true;
        for cursor in &mut self.cursors {
            cursor.insert_text(&self.glyph_brush,&mut self.lines, character);
            cursor.update_cursor(&self.device, &self.glyph_brush, self.scroll as i64  * self.font_scale as i64, &self.lines);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output_frame = self.surface.get_current_texture()?;

        let view = output_frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // create the command buffer
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") } );

        let top_margin_offset = self.top_margin.rect.px_size.1 as i64;
         // draw cursor
        { // to cause _render_pass to be destroyed before self.queue.submit().
            // create a render pass out of the encoder
            let bg_color = rgb(63, 78, 79);
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { // create one attachment for this render pass
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear( wgpu::Color{r:bg_color.0 as f64,g:bg_color.1 as f64,b:bg_color.2 as f64, a :1.0} ), store: true },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.rect_pipeline.pipeline);
            // self.draw(&mut render_pass);
            for cursor in &mut self.cursors {
                cursor.rect.set_offset(&self.device, (0,top_margin_offset));
                cursor.rect.draw(&mut render_pass);
            }
            
            for rect in &self.rectangles {
                rect.draw(&mut render_pass);
            }

            // self.top_margin.rect.draw(&mut render_pass);
            // self.top_margin.left_icon.draw(&mut render_pass);
            self.top_margin.draw(&self.device,&mut render_pass,self.modified);
        }

        // ------------- Draw text ------------------
        // queue text draw
        let mut y_acc = 0; // y position in lines.
        let offset = self.scroll as i64 * self.font_scale as i64;
        for i in 0..self.lines.len() {
            
            let break_num = &self.lines[i].breaks.len();
            for wrap in 0..break_num-1 {
                let pos = (0.0, (y_acc * self.font_scale as i64 - offset + top_margin_offset) as f32);
                
                let text_color = rgb(220, 215, 201);
                //eww
                let text = Text::new(&self.lines[i].text[self.lines[i].breaks[wrap] .. self.lines[i].breaks[wrap+1]]).with_color([text_color.0,text_color.1,text_color.2,1.1]).with_scale(self.font_scale);
                self.glyph_brush.queue(Section {
                    screen_position: pos,
                    bounds: (self.size.width as f32, self.size.height as f32),
                    text: vec![text],
                    layout: wgpu_glyph::Layout::default_single_line(),
                    
                    // ..Section::default() // line ending and v-h align
                });

                y_acc += 1;
            }

        }

        { // Draw margin text===========
            let text_color =rgb(245, 239, 230);
            let pos = ((self.top_margin.rect.px_size.0 / 2) as f32, 2.0);
            let text = Text::new(&self.top_margin.file_name).with_color([text_color.0,text_color.1,text_color.2,1.1]).with_scale(self.font_scale);
            self.glyph_brush.queue(Section {
                screen_position: pos,
                bounds: (self.size.width as f32, self.size.height as f32),
                text: vec![text],
                layout: wgpu_glyph::Layout::default_single_line(),
                
                // ..Section::default() // line ending and v-h align
            });
        }//============================

       
        
        // draw text
        match self.glyph_brush.draw_queued(&self.device, &mut self.staging_belt, &mut encoder, &view, self.size.width, self.size.height) {
            Ok(_) => {},
            Err(e) => println!("error! : {}", e),
        }
        self.staging_belt.finish();
        //------------------------------------------

        self.queue.submit(std::iter::once(encoder.finish())); // do the action of clearing the frame with the color.
        output_frame.present();// present to the screen.

        self.staging_belt.recall(); // wtf does this do.
        Ok(())
    }
}

pub async fn run() {
    // parse user input from cli
    let file_name : String = if let Some(file) = std::env::args().nth(1) {
        file
    }else {
        String::from("untitled.txt")
    };

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_title("mini text").with_decorations(false).build(&event_loop).unwrap();


    let mut state = State::new(&window,file_name).await;

    // unused
    let mut mod_state : ModifiersState = ModifiersState::default();

    let mut cursor_pos = (0,0);    

    event_loop.run(move |event, _, control_flow| match event {
        Event::RedrawRequested(window_id) if window_id == window.id() => {
            state.update();
            match state.render() {
                Ok(_) => {},
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                Err(e) => eprintln!("{:?}",e)
            }
        },
        Event::MainEventsCleared => {
            // Event::RedrawRequested will only run once, unless we request it.
            // This is super slow! Maybe should give control to system now using a target fps thing.
            // window.request_redraw();
        }

        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => if !state.input(event) {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent::Resized(size) => {
                    state.resize(*size);
                },
                WindowEvent::ScaleFactorChanged{new_inner_size,..} => {
                    state.resize(**new_inner_size);
                },

                WindowEvent::KeyboardInput {input: KeyboardInput{state : ElementState::Pressed, virtual_keycode ,..},..} => {
                    if let Some(code) = *virtual_keycode {
                        use VirtualKeyCode::*;
                        match code {
                            Left => {
                                state.move_cursor(CursorMovement::Left);
                                window.request_redraw();
                            }
                            Right => {
                                state.move_cursor(CursorMovement::Right);
                                window.request_redraw();
                            }
                            Up => {
                                state.move_cursor(CursorMovement::Up);
                                window.request_redraw();
                            }
                            Down => {
                                state.move_cursor(CursorMovement::Down);
                                window.request_redraw();
                            }
                            _ => {}
                        }
                    }
                }

                // // keyboard stuff
                WindowEvent::ReceivedCharacter(character) => {
                    if *character == '\u{13}' {
                        // if mod_state.ctrl() && *character == 's' {
                        // Save time!
                        state.save_file();
                    }else{
                        state.insert_cursor(*character);
                    }


                    window.request_redraw();
                }

                // modifiers
                WindowEvent::ModifiersChanged(mods) => {
                    mod_state = *mods;
                }
                

                // Mouse stuff -------------
                WindowEvent::MouseInput { device_id, state: el_state, button, modifiers } => {
                    if let MouseButton::Left = *button {
                        if let ElementState::Released = *el_state {

                            // Go through all buttons
                            if state.top_margin.exit_button.does_click(cursor_pos) {
                                control_flow.set_exit();
                            }

                        }
                    }
                }
                WindowEvent::CursorMoved { device_id, position, modifiers } => {
                    cursor_pos = (position.x as u32,position.y as u32);
                }
                WindowEvent::MouseWheel { device_id, delta, phase, .. }  => {
                    // scroll!
                    match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            // mouse scroll wheel scrolling
                            state.scroll -= *y as f64;
                            for cursor in &mut state.cursors {
                                // TODO: Remove offset from the Cursor struct.
                                // cursor.rect.set_offset(&state.device, (0,(state.scroll as f32 * state.font_scale) as i64));
                                cursor.update_cursor(&state.device,&state.glyph_brush, state.scroll as i64 * state.font_scale as i64, &state.lines);
                            }
                            println!("Scrolling lines ({},{})",x,y);
                        },
                        MouseScrollDelta::PixelDelta( PhysicalPosition{x,y}) => {
                            // mouse pad scrolling
                            state.scroll -= *y as f64;
                            for cursor in &mut state.cursors{
                                cursor.update_cursor(&state.device,&state.glyph_brush, state.scroll as i64 * state.font_scale as i64, &state.lines);
                            }
                            println!("Scrolling pixels ({},{})",x,y);
                        },
                    }
                    window.request_redraw();
                }
                // -------------------------
                _ => {}
            }
        },
        _ => {}
    });
}

 

 

fn main() {
    pollster::block_on(run());
    println!("Hello, world!");
}
