use wasm_bindgen::{Clamped, JsValue};
use web_sys::{CanvasRenderingContext2d, ImageData};

fn fixed_mod(a: f32, b: f32) -> f32 {
    ((a % b) + b) % b
}

#[derive(Copy, Clone)]
enum Field {
    VelocityX,
    VelocityY,
    Permeability,
    SmokeDensity,
}

pub struct Space {
    ctx: CanvasRenderingContext2d,
    num_cells_x: i32,
    num_cells_y: i32,
    velocities_x: Vec<f32>,
    velocities_y: Vec<f32>,
    permeabilities: Vec<f32>,
    smoke_densities: Vec<f32>,
    overrelaxation: f32,
    projection_iterations: i32,
    image_data_vec: Vec<u8>,
    dt: f32,
}

impl Space {
    pub fn new(ctx: CanvasRenderingContext2d, num_cells_x: i32, num_cells_y: i32, dt: f32) -> Self {
        let velocities_x = vec![0.; ((num_cells_x + 1) * num_cells_y) as usize];
        let velocities_y = vec![0.; (num_cells_x * (num_cells_y + 1)) as usize];
        let permeabilities = vec![1.; ((num_cells_x + 2) * (num_cells_y + 2)) as usize];
        let smoke_densities = vec![0.; (num_cells_x * num_cells_y) as usize];

        let image_data_vec = vec![
            0_u8;
            (ctx.canvas().unwrap().width() * ctx.canvas().unwrap().height() * 4)
                as usize
        ];

        let mut space = Space {
            ctx,
            num_cells_x,
            num_cells_y,
            velocities_x,
            velocities_y,
            permeabilities,
            smoke_densities,
            overrelaxation: 1.9,
            projection_iterations: 100,
            image_data_vec,
            dt,
        };

        let cx: i32 = 50;
        let cy: i32 = 50;
        let r: i32 = 10;

        for x in 0..num_cells_x {
            for y in 0..num_cells_y {
                if (x - cx).pow(2) + (y - cy).pow(2) < r.pow(2) {
                    *space.sample_exact_mut(x, y, Field::Permeability) = 0.;
                }
            }
        }

        space
    }

    fn sample_exact(&self, x: i32, y: i32, field: Field) -> f32 {
        match field {
            Field::VelocityX => *self
                .velocities_x
                .get((y * (self.num_cells_x + 1) + x) as usize)
                .unwrap(),
            Field::VelocityY => *self
                .velocities_y
                .get((y * self.num_cells_x + x) as usize)
                .unwrap(),
            Field::Permeability => *self
                .permeabilities
                .get(((y + 1) * (self.num_cells_x + 2) + x + 1) as usize)
                .unwrap(),
            Field::SmokeDensity => *self
                .smoke_densities
                .get((y * self.num_cells_x + x) as usize)
                .unwrap(),
        }
    }

    fn sample_exact_with_bounds_check(&self, x: i32, y: i32, field: Field) -> f32 {
        if x < 0 || y < 0 {
            return 0.0;
        }

        match field {
            Field::VelocityX => {
                if x >= self.num_cells_x + 1 || y >= self.num_cells_y {
                    return 0.0;
                }

                *self
                    .velocities_x
                    .get((y * (self.num_cells_x + 1) + x) as usize)
                    .unwrap()
            }
            Field::VelocityY => {
                if x >= self.num_cells_x || y >= self.num_cells_y + 1 {
                    return 0.0;
                }

                *self
                    .velocities_y
                    .get((y * self.num_cells_x + x) as usize)
                    .unwrap()
            }
            Field::Permeability => {
                if x >= self.num_cells_x + 2 || y >= self.num_cells_y + 2 {
                    return 0.0;
                }

                *self
                    .permeabilities
                    .get(((y + 1) * (self.num_cells_x + 2) + x + 1) as usize)
                    .unwrap()
            }
            Field::SmokeDensity => {
                if x >= self.num_cells_x || y >= self.num_cells_y {
                    return 0.0;
                }

                *self
                    .smoke_densities
                    .get((y * self.num_cells_x + x) as usize)
                    .unwrap()
            }
        }
    }

    fn sample_exact_mut(&mut self, x: i32, y: i32, field: Field) -> &mut f32 {
        match field {
            Field::VelocityX => self
                .velocities_x
                .get_mut((y * (self.num_cells_x + 1) + x) as usize)
                .unwrap(),
            Field::VelocityY => self
                .velocities_y
                .get_mut((y * self.num_cells_x + x) as usize)
                .unwrap(),
            Field::Permeability => self
                .permeabilities
                .get_mut(((y + 1) * (self.num_cells_x + 2) + x + 1) as usize)
                .unwrap(),
            Field::SmokeDensity => self
                .smoke_densities
                .get_mut((y * self.num_cells_x + x) as usize)
                .unwrap(),
        }
    }

    fn sample_interp(&self, mut x: f32, mut y: f32, field: Field) -> f32 {
        match field {
            Field::Permeability => {
                x += 1.0;
                y += 1.0;
            }
            Field::VelocityX => {
                x += 0.5;
            }
            Field::VelocityY => {
                y += 0.5;
            }
            Field::SmokeDensity => (),
        }

        let ix = x as i32;
        let iy = y as i32;
        let fx = fixed_mod(x, 1.0);
        let fy = fixed_mod(y, 1.0);

        let c00 = self.sample_exact_with_bounds_check(ix, iy, field.clone());
        let c10 = self.sample_exact_with_bounds_check(ix + 1, iy, field.clone());
        let c01 = self.sample_exact_with_bounds_check(ix, iy + 1, field.clone());
        let c11 = self.sample_exact_with_bounds_check(ix + 1, iy + 1, field.clone());

        let c0 = c00 * (1. - fx) + c10 * fx;
        let c1 = c01 * (1. - fx) + c11 * fx;

        c0 * (1. - fy) + c1 * fy
    }

    fn project(&mut self) {
        for iy in 0..self.num_cells_y {
            for ix in 0..self.num_cells_x {
                let su0 = self.sample_exact(ix - 1, iy, Field::Permeability);
                let su1 = self.sample_exact(ix + 1, iy, Field::Permeability);
                let sv0 = self.sample_exact(ix, iy - 1, Field::Permeability);
                let sv1 = self.sample_exact(ix, iy + 1, Field::Permeability);
                let s = su1 + su0 + sv1 + sv0;

                if s == 0.0 {
                    continue;
                }

                let u0 = self.sample_exact(ix, iy, Field::VelocityX);
                let u1 = self.sample_exact(ix + 1, iy, Field::VelocityX);
                let v0 = self.sample_exact(ix, iy, Field::VelocityY);
                let v1 = self.sample_exact(ix, iy + 1, Field::VelocityY);
                let d = self.overrelaxation * (u1 - u0 + v1 - v0);

                *self.sample_exact_mut(ix, iy, Field::VelocityX) += d * su0 / s;
                *self.sample_exact_mut(ix + 1, iy, Field::VelocityX) -= d * su1 / s;
                *self.sample_exact_mut(ix, iy, Field::VelocityY) += d * sv0 / s;
                *self.sample_exact_mut(ix, iy + 1, Field::VelocityY) -= d * sv1 / s;
            }
        }
    }

    fn advect_velocities(&mut self) {
        let mut velocities_copy_x =
            Vec::<f32>::with_capacity(((self.num_cells_x + 1) * self.num_cells_y) as usize);
        let mut velocities_copy_y =
            Vec::<f32>::with_capacity((self.num_cells_x * (self.num_cells_y + 1)) as usize);

        for iy in 0..self.num_cells_y {
            for ix in 0..(self.num_cells_x + 1) {
                let real_x = ix as f32 - 0.5;
                let real_y = iy as f32;

                let vx = self.sample_exact(ix, iy, Field::VelocityX);
                let vy = self.sample_interp(real_x, real_y, Field::VelocityY);

                let prev_x = real_x - vx * self.dt;
                let prev_y = real_y - vy * self.dt;

                let prev_vx = self.sample_interp(prev_x, prev_y, Field::VelocityX);
                velocities_copy_x.push(prev_vx);
            }
        }

        for iy in 0..(self.num_cells_y + 1) {
            for ix in 0..self.num_cells_x {
                let real_x = ix as f32;
                let real_y = iy as f32 - 0.5;

                let vx = self.sample_interp(real_x, real_y, Field::VelocityX);
                let vy = self.sample_exact(ix, iy, Field::VelocityY);

                let prev_x = real_x - vx * self.dt;
                let prev_y = real_y - vy * self.dt;

                let prev_vy = self.sample_interp(prev_x, prev_y, Field::VelocityY);
                velocities_copy_y.push(prev_vy);
            }
        }

        for i in 0..self.velocities_x.len() {
            *self.velocities_x.get_mut(i).unwrap() = *velocities_copy_x.get(i).unwrap();
        }

        for i in 0..self.velocities_y.len() {
            *self.velocities_y.get_mut(i).unwrap() = *velocities_copy_y.get(i).unwrap();
        }
    }

    fn advect_smoke(&mut self) {
        let mut smoke_densities_copy = Vec::<f32>::with_capacity(self.smoke_densities.len());

        for iy in 0..self.num_cells_y {
            for ix in 0..self.num_cells_x {
                let real_x = ix as f32;
                let real_y = iy as f32;

                let vx = self.sample_interp(real_x, real_y, Field::VelocityX);
                let vy = self.sample_interp(real_x, real_y, Field::VelocityY);

                let prev_x = real_x - vx * self.dt;
                let prev_y = real_y - vy * self.dt;

                let prev_smoke_density = self.sample_interp(prev_x, prev_y, Field::SmokeDensity);
                smoke_densities_copy.push(prev_smoke_density);
            }
        }

        for i in 0..self.smoke_densities.len() {
            *self.smoke_densities.get_mut(i).unwrap() = *smoke_densities_copy.get(i).unwrap();
        }
    }

    fn add_smoke(&mut self) {
        let lo = 9 * self.num_cells_y / 20;
        let hi = 11 * self.num_cells_y / 20;

        for x in 0..15 {
            for y in lo..hi {
                *self.sample_exact_mut(x, y, Field::SmokeDensity) = 1.;
            }
        }

        for x in 3..5 {
            for y in 0..self.num_cells_y {
                *self.sample_exact_mut(x + 1, y, Field::VelocityX) = 400.;
            }
        }
    }

    pub fn step(&mut self) {
        for _ in 0..self.projection_iterations {
            self.project();
        }

        self.advect_velocities();
        self.advect_smoke();

        self.add_smoke();
    }

    pub fn render(&mut self) {
        let canvas_width = self.ctx.canvas().unwrap().width();
        let canvas_height = self.ctx.canvas().unwrap().height();

        for y in 0..canvas_height {
            let simulation_y = (y as f32) / (canvas_height as f32) * (self.num_cells_y as f32);

            for x in 0..canvas_width {
                let simulation_x = (x as f32) / (canvas_width as f32) * (self.num_cells_x as f32);

                let ix = simulation_x as i32;
                let iy = simulation_y as i32;
                let smoke_density = self.sample_exact(ix, iy, Field::SmokeDensity);

                let index = (y * canvas_width * 4 + x * 4) as usize;
                let color = ((1. - smoke_density) * 255.0) as u8;

                if self.sample_exact(ix, iy, Field::Permeability) == 0.0 {
                    *self.image_data_vec.get_mut(index + 0).unwrap() = 50;
                    *self.image_data_vec.get_mut(index + 1).unwrap() = 168;
                    *self.image_data_vec.get_mut(index + 2).unwrap() = 82;
                    *self.image_data_vec.get_mut(index + 3).unwrap() = 255;
                } else {
                    *self.image_data_vec.get_mut(index + 0).unwrap() = color;
                    *self.image_data_vec.get_mut(index + 1).unwrap() = color;
                    *self.image_data_vec.get_mut(index + 2).unwrap() = color;
                    *self.image_data_vec.get_mut(index + 3).unwrap() = 255;
                }
            }
        }

        let data: Clamped<&[u8]> = Clamped::<&[u8]>(self.image_data_vec.as_mut_slice());
        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(data, canvas_width, canvas_height).unwrap();
        self.ctx.put_image_data(&image_data, 0.0, 0.0).unwrap();

        self.ctx.begin_path();
        self.ctx.set_stroke_style(&JsValue::from_str("#000000"));

        for y in (0..self.num_cells_y).step_by(2) {
            for x in (0..self.num_cells_x).step_by(2) {
                let real_x = x as f32;
                let real_y = y as f32;

                let vx = self.sample_interp(real_x, real_y, Field::VelocityX) as f64;
                let vy = self.sample_interp(real_x, real_y, Field::VelocityY) as f64;

                let canvas_x =
                    ((real_x + 0.5) * (canvas_width as f32) / (self.num_cells_x as f32)) as f64;
                let canvas_y =
                    ((real_y + 0.5) * (canvas_height as f32) / (self.num_cells_y as f32)) as f64;

                self.ctx.move_to(canvas_x, canvas_y);
                // self.ctx.line_to(canvas_x + vx * 0.1, canvas_y + vy * 0.1);
            }
        }

        self.ctx.stroke();
    }
}
