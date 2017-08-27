use std::fs::File;
use std::io::{BufRead, BufReader, Lines, Read};
use std::mem;

const BYTES_IN_A_C_FLOAT : usize = 4;
const NEIGHBOR_DISTANCE2 : f32   = 10.0; ////7.5625;.


#[derive(Clone, Debug)]
struct Parameters {
    particles_no  : usize,
    steps         : usize,
    exit_interval : usize,
    dimensions    : usize,
    dt            : f32,
    range         : f32,
}

#[derive(Clone, Debug)]
struct FileLine {
    positions    : Vec<f32>,
    cell_type    : u16,
    neighbors_no : u16,
    core_size    : f32,
}

impl Parameters {
    fn exit_steps(&self) -> usize {
        (self.steps / self.exit_interval) + 1
    }
    fn new() -> Parameters {
        Parameters {
            particles_no   : 0,
            steps         : 0,
            exit_interval : 0,
            dimensions    : 0,
            dt            : -0.0,
            range         : -0.0,
        }
    }

    fn read(&mut self, file_buffer : &mut BufReader<&File>) -> Parameters {
        fn unwrap_several_times(my_lines : &mut Lines<&mut BufReader<&File>>) -> String {
            my_lines
                .nth(0)
                .unwrap()
                .unwrap()
                .to_string()
        }

        let mut lines = file_buffer.lines();
        self.particles_no = unwrap_several_times(&mut lines)
            .parse::<usize>()
            .unwrap();
        self.steps = unwrap_several_times(&mut lines)
            .parse::<usize>()
            .unwrap();
        self.exit_interval = unwrap_several_times(&mut lines)
            .parse::<usize>()
            .unwrap();
        self.dimensions = unwrap_several_times(&mut lines)
            .parse::<usize>()
            .unwrap();
        self.dt = unwrap_several_times(&mut lines)
            .parse::<f32>()
            .unwrap();
        self.range = unwrap_several_times(&mut lines)
            .parse::<f32>()
            .unwrap();
        self.clone()
    }
}

fn read_positions(parameters   : &Parameters,
                  in_buffer    : &mut BufReader<&File>,
                  initial_step : usize,
                  final_step   : usize,) -> Result<(Vec<FileLine>, Vec<FileLine>), String> {
    if initial_step == parameters.steps {
        return Err("Start step equals to final step.".to_string());
    }
    
    let mut initial_step_vec : Vec<FileLine> = Vec::new();
    let mut final_step_vec   : Vec<FileLine> = Vec::new();
    
    for step in 0..parameters.exit_steps() {
        let is_initial = {
            if step * parameters.exit_interval == initial_step {
                true
            } else {
                false
            }
        };
        let is_final = {
            if step * parameters.exit_interval == final_step {
                true
            } else {
                false
            }
        };
        
        for _ in 0..parameters.particles_no {
            let mut line = FileLine {
                positions    : Vec::new(),
                cell_type    : std::u16::MAX,
                neighbors_no : std::u16::MAX,
                core_size    : -0.0,
            };
            for _ in 0..parameters.dimensions {
                let mut slice = [0u8; BYTES_IN_A_C_FLOAT];
                match in_buffer
                    .read_exact(&mut slice) {
                        Err(err) => { /*println!("{}", step);*/ return Err(err.to_string() + " (position_component)"); }
                        Ok(())   => { }
                    }
                let banana = unsafe {
                    mem::transmute::<[u8; BYTES_IN_A_C_FLOAT], f32>(slice)
                };
                if is_initial || is_final {
                    line.positions.push(banana);
                }
            }
            for extra_info_no in 0..2 {
                let mut slice = [0u8; 2];
                match in_buffer
                    .read_exact(&mut slice) {
                        Err(e) => { return Err(e.to_string() + " (extra info)"); }
                        _ => { }
                    }
                let papaya = unsafe {
                    mem::transmute::<[u8; 2], u16>(slice)
                };
                if is_initial || is_final {
                    match extra_info_no {
                        0 => line.cell_type    = papaya,
                        1 => line.neighbors_no = papaya,
                        _ => { return Err("Binary must be v3.".to_string()) }
                    }
                    
                }
            }
            {
                let mut slice = [0u8; BYTES_IN_A_C_FLOAT];
                match in_buffer
                    .read_exact(&mut slice) {
                        Err(e) => { return Err(e.to_string() + "( core size)"); }
                        _      => { }
                    }
                let raspberry = unsafe {
                    mem::transmute::<[u8; BYTES_IN_A_C_FLOAT], f32>(slice)
                };  
                if is_initial || is_final {
                    line.core_size = raspberry;
                }
            }   
            if is_initial {
                initial_step_vec.push(line.clone());
            } else if is_final {
                final_step_vec.push(line.clone());
            }                
        }
    }
    
    Ok((initial_step_vec, final_step_vec))
}

fn get_cell_neighbors(parameters : &Parameters,
                      initial_vec : &Vec<FileLine>,
                      my_cell_index : &usize) -> Result<Vec<(usize, f32)>, String> {
    let mut neighbors_vec = Vec::new();
    for cell_index in 0..parameters.particles_no {
        if cell_index != *my_cell_index {
            let distance2 = {
                let mut sum2 = -0.0f32;
                for dimension_index in 0..parameters.dimensions {
                    sum2 += (initial_vec[*my_cell_index].positions[dimension_index] -
                             initial_vec[cell_index]    .positions[dimension_index])
                        .powi(2); // banana.powi(2) = banana**2
                }
                sum2
            };

            if distance2 < NEIGHBOR_DISTANCE2 {
                neighbors_vec.push((cell_index, distance2));
            }
        }
    }
    
    Ok(neighbors_vec)
}

fn get_delta(parameters : &Parameters, initial_vec : &Vec<FileLine>, final_vec : &Vec<FileLine>) -> f32 {
    let mut delta = -0.0f32;
    let mut particles_with_neighbors_no : usize = 0;
    for my_cell_index in 0..parameters.particles_no {
        let mut sum = -0.0f32;
        let cell_neighbors = &match get_cell_neighbors(&parameters, &initial_vec, &my_cell_index) {
            Ok(vec) => vec,
            Err(e)  => panic!("{}", e.to_string()),
        };
        if cell_neighbors.len() != 0 {
            particles_with_neighbors_no += 1;
            for &(index, initial_distance2) in cell_neighbors {
                let final_distance2 = {
                    let mut sum = -0.0;
                    for dimension_index in 0..parameters.dimensions {
                        sum += (final_vec[my_cell_index]
                                .positions[dimension_index] -
                                final_vec[index]
                                .positions[dimension_index])
                            .powi(2); // banana.powi(2) = banana**2
                    }
                    sum
                };
                sum += (1.0 - (initial_distance2 / final_distance2)) / cell_neighbors.len() as f32;
            }
        }
        if particles_with_neighbors_no != 0 {
            let my_delta = sum;
            delta += my_delta;
        }
    }
    delta / particles_with_neighbors_no as f32
}

fn main() {
    let file = match std::env::args()
        .nth(1)
        .map(|filename| File::open(filename)) {
        Some(o) => {
            match o {
                Ok(r)  => { r }
                Err(err) => panic!("{}", err.to_string())
            }
        }
        None    => panic!("No input file name.")
    };
    let initial_step = match std::env::args()
        .nth(2)
        .map(|step| step.parse::<usize>()) {
            Some(s) => {
                match s {
                    Ok(r)    => r,
                    Err(err) => panic!("{}", err.to_string()),
                }
            },
            None => panic!("No input initial step."),
        };
    let big_t = match std::env::args()
        .nth(3)
        .map(|step| step.parse::<usize>()) {
            Some(s) => {
                match s {
                    Ok(r)    => r,
                    Err(err) => panic!("{}", err.to_string()),
                }
            },
            None => panic!("No input T steps interval."),
        };
    
    
    let mut file_buffer = BufReader::new(&file);
    let parameters = &Parameters::new()
        .read(&mut file_buffer);
    
    let (initial_vec, final_vec) = match read_positions(parameters,
                                                        &mut file_buffer,
                                                        initial_step,
                                                        initial_step + big_t,) {
        Err(e)    => { panic!("{}", e.to_string()); }
        Ok(tuple) => { tuple }
    };
    
    let delta = get_delta(parameters, &initial_vec, &final_vec);
    if delta >= 1.0 {
        eprintln!("caca\t{}", delta);
    }
    println!("{}", delta);
}
