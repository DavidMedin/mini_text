use wgpu::{include_wgsl, util::DeviceExt};

pub struct RectPipeline {
    pub pipeline : wgpu::RenderPipeline
}
impl RectPipeline {
    pub fn new(device : &wgpu::Device, format : wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module( include_wgsl!("rect.wgsl") );

        let pipeline_layout = device.create_pipeline_layout( &wgpu::PipelineLayoutDescriptor{
            label: Some("Rectangle Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // code vomit
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label: Some("Rectangle Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers : &[Vertex::desc()] // get a description from the Vertex type, the type that
                            // will be in the buffer anyway.
            },
            fragment: Some(wgpu::FragmentState{
                module : &shader,
                entry_point: "fs_main",
                targets : &[
                    Some(wgpu::ColorTargetState{
                        format:format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL
                    })
                ]
            }),

            primitive: wgpu::PrimitiveState{
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format : None,
                front_face : wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back), // does this slow us down if we never need to cull?
                polygon_mode: wgpu::PolygonMode::Fill, // TODO: Lines?
                unclipped_depth : false,
                conservative : false
            },

            depth_stencil: None,
            multisample: wgpu::MultisampleState{
                count : 1,
                mask: !0,
                alpha_to_coverage_enabled : false
            },
            multiview: None,
        });


        RectPipeline{ pipeline: render_pipeline }
    }
}


#[repr(C)]
#[derive(Copy,Clone,Debug, bytemuck::Pod, bytemuck::Zeroable)] // bytemuck so it can be converted to an array of bytes
struct Vertex{
    position: [f32; 2],
    color: [f32;3],
}
impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute{
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format : wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<[f32;2]>() as wgpu::BufferAddress,
                    shader_location : 1
                }
            ],
        }
    }
}

// x,y,w,h (screen space) -> x,y,w,h (world space)
fn world_space(screen_size : (u32,u32),x:u32,y:u32,width : u32,height:u32) -> (f32,f32,f32,f32) {
    (
        x as f32 /(screen_size.0 as f32 / 2.0) - 1.0,
        -1.0 * (y as f32/(screen_size.1 as f32 / 2.0) - 1.0),
        width as f32/screen_size.0 as f32 * 2.0,
        height as f32/screen_size.1 as f32 * 2.0
    )
}
fn screen_space(screen_size: (u32,u32),x:f32,y:f32,width:f32,height:f32) -> (u32,u32,u32,u32) {
    (
        ( (x+1.0) * (screen_size.0 as f32 / 2.0) ) as u32,
        ( ( (-y+1.0) * (screen_size.1 as f32 / 2.0) ) as u32),
        (width * screen_size.0 as f32 / 2.0) as u32,
        (height * screen_size.1 as f32 / 2.0) as u32
    )
}

pub struct Rect {
    vertices : [Vertex;6],
    size :(f32,f32),
    pos : (f32,f32),
    
    offset : (u32,u32),
    px_size: (u32,u32),
    px_pos: (u32,u32),
    
    color : (f32,f32,f32),
    screen_size : (u32,u32),
    
    pub vertex_buffer : wgpu::Buffer,

}
impl Rect {
    pub fn new(device : &wgpu::Device, screen_size : (u32,u32), size : (u32,u32),pos : (u32,u32), offset : (u32,u32), color:(f32,f32,f32)) -> Self {
        let px_pos = pos;
        let px_size = size;
        let (x,y,width,height) = world_space(size,pos.0,pos.1,size.0,size.1);

        let vertices : &[Vertex;6] = &[
            Vertex { position: [x,y], color: [color.0,color.1,color.2] },
            Vertex{ position : [x,y-height], color : [color.0,color.1,color.2]},
            Vertex{ position : [x+width,y-height], color : [color.0,color.1,color.2]},

            Vertex {position : [x+width,y], color : [color.0,color.1,color.2]},
            Vertex { position: [x,y], color: [color.0,color.1,color.2] },
            Vertex{ position : [x+width,y-height], color : [color.0,color.1,color.2]},

        ];
        
        let vertex_buffer = device.create_buffer_init( &wgpu::util::BufferInitDescriptor{
            label: Some("Rect Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Rect{ vertices : *vertices, size : (width,height), pos: (x,y), vertex_buffer, screen_size : size, color , px_pos, px_size, offset }
    }

    pub fn draw<'a>(&'a self,render_pass : & mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..6,0..1);
    }

    pub fn get_pos(&self)->(u32,u32) {
        let (x,y,_,_) = screen_space(self.screen_size, self.pos.0, self.pos.1, self.size.0, self.size.1);
        (x,y)
    }
    pub fn update_rect(&mut self,device : &wgpu::Device, screen_size : (u32,u32)){
        // use the old screen size to get the old pixel placement
        let(x,y,w,h) = world_space(screen_size, self.px_pos.0, self.px_pos.1, self.px_size.0, self.px_size.1);

        self.screen_size = screen_size;
        self.pos = (x,y);
        self.size = (w,h);

        let color = [self.color.0,self.color.1,self.color.2];
        let vertices : &[Vertex;6] = &[
            Vertex { position: [x,y], color },
            Vertex{ position : [x,y-h], color},
            Vertex{ position : [x+w,y-h], color},

            Vertex {position : [x+w,y], color},
            Vertex { position: [x,y], color },
            Vertex{ position : [x+w,y-h], color},

        ];

        self.vertex_buffer = device.create_buffer_init( &wgpu::util::BufferInitDescriptor{
            label: Some("Rect Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
    }

    pub fn set_rect(&mut self,device : & wgpu::Device, x:u32,y:u32,w:u32,h:u32) {
        self.px_pos = (x,y);
        self.px_size = (w,h);
        let (x,y,w,h) = world_space(self.screen_size, x + self.offset.0, y + self.offset.1, w, h);
        self.pos = (x,y);
        self.size = (w,h);

        let color = [self.color.0,self.color.1,self.color.2];
        let vertices : &[Vertex;6] = &[
            Vertex { position: [x,y], color },
            Vertex{ position : [x,y-h], color},
            Vertex{ position : [x+w,y-h], color},

            Vertex {position : [x+w,y], color},
            Vertex { position: [x,y], color },
            Vertex{ position : [x+w,y-h], color},

        ];

        self.vertex_buffer = device.create_buffer_init( &wgpu::util::BufferInitDescriptor{
            label: Some("Rect Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
    }

    pub fn set_pos(&mut self,device : & wgpu::Device, pos : (u32,u32), offset : (u32,u32)) {
        self.px_pos = pos;
        let (x,y,w,h) = world_space(self.screen_size, pos.0 + offset.0,pos.1 + offset.1, self.px_size.0,self.px_size.1);
        self.pos = (x,y);
        self.size = (w,h);

        let color = [self.color.0,self.color.1,self.color.2];
        let vertices : &[Vertex;6] = &[
            Vertex { position: [x,y], color },
            Vertex{ position : [x,y-h], color},
            Vertex{ position : [x+w,y-h], color},

            Vertex {position : [x+w,y], color},
            Vertex { position: [x,y], color },
            Vertex{ position : [x+w,y-h], color},

        ];

        self.vertex_buffer = device.create_buffer_init( &wgpu::util::BufferInitDescriptor{
            label: Some("Rect Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
    }

    pub fn set_offset(&mut self, device : &wgpu::Device, offset : (u32,u32)) {
        self.offset = offset;
        self.set_pos(device, (self.px_pos.0,self.px_pos.1), offset);
    }
}