use std::env;
use std::cmp;
use std::io::prelude::*;
use std::fs::File;

enum EncodingMode {
    Mode1,
    Mode2,
    Mode3,
}

struct BitStream {
    bit_index: u8,
    byte_index: usize,
    bytes: Vec<u8>,
    last_two_bits: u8,
}

impl BitStream {
    fn load_bytes_from_file(&mut self, filename: &str) {
        let mut file = match File::open(&filename) {
            Ok(file) => file,
            Err(error) => panic!("Could not open the file! {:?}", error)
        };
        match file.read_to_end(&mut self.bytes) {
            Ok(_) => println!("File data loaded!"),
            Err(_) => panic!("An error ocurred trying to read the file"),
        };
    }

    fn next_bit(&mut self) {
        self.bit_index += 1;
        self.check_end_of_byte();
        self.update_last_two_bits();
    }

    fn current_byte(&self) -> u8 {
        match self.bytes.get(self.byte_index) {
            Some(byte) => *byte,
            None => 0,
        }
    }

    fn current_bit(&self) -> u8 {
        (self.current_byte() >> 7 - self.bit_index) & 0b00000001
    }

    fn update_last_two_bits(&mut self) {
        self.last_two_bits = ((self.last_two_bits << 1) | self.current_bit()) & 0b00000011;
    }

    fn check_end_of_byte(&mut self) {
        let last_byte_index = self.bytes.len() - 1;
        if self.byte_index >= last_byte_index {
            self.byte_index = last_byte_index;
            if self.bit_index >= 7 {
                self.bit_index = 7;
            }
        } else {
            if self.bit_index > 7 {
                self.bit_index = 0;
                self.byte_index += 1;
                if self.byte_index > last_byte_index {
                    self.byte_index = last_byte_index;
                }
            }
        }
    }

    fn read_bits(&mut self, bits_amount: u8, write_from_left: bool) -> u8 {
        let mut count: u8 = 0;
        let mut byte: u8 = 0;

        while count < bits_amount {
            byte = match write_from_left {
                true => byte | (self.current_bit() << 7 - count),
                false => (byte << 1) | self.current_bit(),
            };

            count += 1;
            self.update_last_two_bits();
            self.next_bit();
        }

        byte
    }

    fn bits_left(&self) -> usize {
        (self.bytes.len() * 8) - ((self.byte_index * 8) + 1 + self.bit_index as usize) as usize
    }
}

struct Buffer {
    bit_index: u8,
    width: u8,
    height: u8,
    vertical_offset: u8,
    horizontal_offset: u8,
    byte_index: usize,
    bytes: Vec<u8>,
    bitplane_length: usize,
    row_index: usize,
}

impl Buffer {
    fn allocate_space(&mut self, width: u8, height: u8) {
        const MAX_SPRITE_SIZE: u8 = 7; // 7 tiles
        self.width = width;
        self.height = height;
        // We will need the vertical and horizontal offsets later, this is used to center the resulting
        // sprite in a box of 7 * 7 tiles
        // vertical offset = 7 - height
        // horizontal offset = ((7 - width) / 2) + (1/2) -> then round the result down
        self.vertical_offset = MAX_SPRITE_SIZE - width;
        self.horizontal_offset = ((MAX_SPRITE_SIZE - height) / 2) + (1 / 2);
        // We need 3 bitplanes, the first and second ones are where the 
        // decompressed bytes will be, which are 7 x 7 each.
        // The third one is usually 7 x 7 maximum too, but glitched pokemon could
        // have way more
        // Note: Each tile has 64 pixels
        self.bitplane_length = ((7 * 7 * 2) + cmp::max(7 * 7, width as usize * height as usize)) * 8;
        self.bytes = vec![0; self.bitplane_length];
    }

    fn write_pair(&mut self, data: u8) {
        let column_height = self.height * 8;

        self.bytes[self.byte_index] = self.bytes[self.byte_index] | (data << 8 - (self.bit_index + 2));
        // println!("Byte {:08b}", self.bytes[self.byte_index]);

        self.byte_index += 1;
        self.row_index += 1;
        // We have reached the end of the column
        if self.row_index >= column_height as usize {
            self.row_index = 0;
            self.byte_index -= column_height as usize;
            self.bit_index += 2;// Next column (in bits)
            if self.bit_index >= 8 {
                self.bit_index = 0;
                // Jump to the next column
                self.byte_index += column_height as usize;
            }
        }
    }

    fn write_zero_pairs(&mut self, zero_pairs_amount: usize) {
        let mut count = 0;

        while count < zero_pairs_amount {
            self.write_pair(0);
            count += 1;
        }
    }

    fn decompress_to_bitplane(&mut self, bytes: &mut BitStream, initial_packet: u8, primary_buffer: bool, first_bitplane: bool) {
        let mut rle_length: u8 = 0;
        let mut reading_first_rle = initial_packet == 0; // 1 for data packet and 0 for RLE packet
        let mut reading_second_rle = false;
        let mut first_rle_bits_read: u16 = 0;
        let mut bits_written: usize = 0;
        let bytes_to_write: usize = self.width as usize * self.height as usize * 8;
        // If the primary buffer is true, start decoding into buffer B at location 392,
        // else, decode into buffer C at location 784
        // Glitched pokemons overflow from Buffer B to C
        self.byte_index = if primary_buffer {7 * 7 * 8} else {7 * 7 * 8 * 2};
        self.bit_index = 0;
        self.row_index = 0;

        println!("Bytes to write: {}", bytes_to_write);
        while (first_bitplane && bits_written < bytes_to_write * 8) || (!first_bitplane && bytes.bits_left() > 0) {

            if reading_first_rle {
                let current_bit = bytes.current_bit();
                rle_length += 1; // We have to count the length of the rle packet even if the bit is zero
                if current_bit == 0 {
                    // Once we find a zero, we can start reading the amount of bits we counted
                    // If the bit is 1, we increment the count by 1 and jump to the next bit
                    first_rle_bits_read = (first_rle_bits_read << 1) | (current_bit as u16);
                    reading_first_rle = false;
                    reading_second_rle = true;
                } else {
                    // If the bit is 1, we increment the count by 1 and jump to the next bit
                    first_rle_bits_read = (first_rle_bits_read << 1) | (current_bit as u16);
                }
                bytes.next_bit();
                continue;
            }
            if reading_second_rle {
                // Read the amount of bits we counted
                let mut second_rle_bits_read = 0;
                let mut rle_bits_count = 0;
                while rle_bits_count < rle_length {
                    let current_bit = bytes.current_bit();
                    second_rle_bits_read = (second_rle_bits_read << 1) | (current_bit as u16);
                    rle_bits_count += 1;
                    bytes.next_bit();
                }
                // Then, we add the first 2 groups plus one (the plus one is to take care of
                // the "offset" of the compression algorithm)
                // This is the amount to zero pairs that we have to add to the buffer.
                // For example, if the result is 4, we will have to add 4 zero pairs,
                // or 8 zeros in total
                let zero_pairs = first_rle_bits_read + second_rle_bits_read + 1;
                self.write_zero_pairs(zero_pairs as usize); // write the zero pairs to the buffer
                bits_written += zero_pairs as usize * 2;
                first_rle_bits_read = 0;
                rle_length = 0;
                // Aftrer reading RLE packets, the next is a data packet until we find a 00 pair
                reading_first_rle = false;
                reading_second_rle = false;
                continue;
            }
            // When we are not reading RLE packets, we can read the pairs of data (data packets) until
            // we find a 00 pair
            if !reading_first_rle && !reading_second_rle {
                let bits_pair = bytes.read_bits(2, false);
                if bits_pair == 0 {
                    rle_length = 0;
                    reading_first_rle = true;
                    reading_second_rle = false;
                } else {
                    self.write_pair(bits_pair); // write the zero pairs to the buffer
                    bits_written += 2;
                }
            }
        }
        println!("Bytes written: {}", bits_written / 8);
    }

    fn delta_decode(&mut self, buffer: u8) {
        let index_offset = match buffer{
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };
        let mut row_index: usize = 0;
        let row_height = self.height as usize * 8; // Height in bits
        let col_width = self.width as usize; // Width in bytes
        // The initial state is always zero at the beginning of each row
        let delta_decode_nibble: [u8; 16] = [
            0b0000, 0b0001, 0b0011, 0b0010,
            0b0111, 0b0110, 0b0100, 0b0101,
            0b1111, 0b1110, 0b1100, 0b1101,
            0b1000, 0b1001, 0b1011, 0b1010,
        ];

        // We have to process row by row, then pairs of 4 bits for each column
        while row_index < row_height {
            let mut prev_state = 0;
            let mut col_index: usize = 0;

            while col_index < col_width {
                // Calculate the index in the bytes
                let index: usize = (col_index * (self.height as usize * 8) + row_index) + index_offset;
                let byte = self.bytes[index];

                // Getting the first sub-column (4 bits)
                let first = delta_decode_nibble[(byte >> 4) as usize] ^ (0b1111 * prev_state);
                prev_state = first & 1;

                // Then the second sub-column (4 bits)
                let second = delta_decode_nibble[(byte & 0b1111) as usize] ^ (0b1111 * prev_state);
                prev_state = second & 1;

                // Combine the two
                self.bytes[index] = (first << 4) + second;
                col_index += 1;
            }

            row_index += 1;
            self.byte_index += 1;
        }
    }

    fn xor_buffers(&mut self, buffer_index: u8, replace_buffer: u8) {
        let buffer_index_offset: usize = match buffer_index {
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };
        let replace_index_offset: usize = match replace_buffer {
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };
        let end_index = 8 * 7 * 7;
        let mut index = 0;
        while index < end_index {

            self.bytes[index + replace_index_offset] =
                self.bytes[index + replace_index_offset] ^
                self.bytes[index + buffer_index_offset];

            index += 1;
        }
    }

    fn wipe_bitplane(&mut self, bitplane: u8) {
        let offset: usize = match bitplane {
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };

        // Wipe the "to" bitplane first
        let mut index = offset;
        let buffer_size = 7 * 7 * 8;
        while index < offset + buffer_size {
            self.bytes[index] = 0;
            index += 1;
        }
    }

    fn copy_bitplane(&mut self, from: u8, to: u8) {

        self.wipe_bitplane(to);

        let to_bitplane_start: usize = match to {
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };
        let from_bitplane_start: usize = match from {
            0 => 0, // Address at 0
            1 => 7 * 7 * 8, // Address at 392
            _ => 7 * 7 * 8 * 2, // Address at 784
        };
        let mut from_bitplane_index = from_bitplane_start;

        // Step 1: calculate the offset of the top-left corner
        let mut index: usize = ((self.vertical_offset as usize * 8) + (self.horizontal_offset as usize * 8 * 7)) + to_bitplane_start;

        // Step 2: copy the columns (height) of tiles
        let height = self.height as usize * 8;
        let mut current_column = 0;
        while current_column < self.width {
            let mut row_count: usize = 0;
            while row_count < height {
                self.bytes[index] = self.bytes[from_bitplane_index];
                index += 1;
                from_bitplane_index += 1;
                row_count += 1;
            }

            // Revert the pointer back to the previous offset and add 56
            // This will put the pointer to the next offset vertical offset
            index -= height;
            index += 56;

            current_column += 1;
        }
    }

    fn zip_buffers(&mut self) {
        let mut last_index_buffer_a: usize = (7 * 7 * 8) - 1;
        let mut last_index_buffer_b: usize = (7 * 7 * 8 * 2) - 1;
        let mut last_index_buffer_c: usize = (7 * 7 * 8 * 3) - 1;

        println!("last index A: {}", last_index_buffer_a);
        println!("last index B: {}", last_index_buffer_b);
        println!("last index C: {}", last_index_buffer_c);

        loop {
            self.bytes[last_index_buffer_c] = self.bytes[last_index_buffer_b];
            last_index_buffer_c -= 1;
            self.bytes[last_index_buffer_c] = self.bytes[last_index_buffer_a];

            if last_index_buffer_a == 0 {
                break;
            }

            last_index_buffer_a -= 1;
            last_index_buffer_b -= 1;
            last_index_buffer_c -= 1;
        }

        println!("Buffer C result: {:02X?}", &self.bytes[last_index_buffer_c..]);
    }

    fn render(&self) {
        let mut pixels = Vec::<u8>::new(); // a "vram" to store each pixel
        let buffer_b_start = 7 * 7 * 8; // 392
        let mut index = buffer_b_start;
        let buffer_c_end = (7 * 7 * 8 * 3) - 1; // 1175

        while index <= buffer_c_end {
            let mut bit_index = 0;

            while bit_index <= 7 {
                let bit_a = (self.bytes[index] >> (7 - bit_index)) & 0b00000001;
                let bit_b = (self.bytes[index + 1] >> (7 - bit_index)) & 0b00000001;

                if bit_a == 0 && bit_b == 0 {
                    pixels.push(0);
                } else if bit_a == 0 && bit_b == 1 {
                    pixels.push(1);
                } else if bit_a == 1 && bit_b == 0 {
                    pixels.push(2);
                } else {
                    pixels.push(3);
                }

                bit_index += 1;
            }

            index += 2;
        }

        println!("{}", termion::clear::All);
        let mut pixel_col = 0;
        let mut pixel_row = 0;
        let mut current_row_col = 0;
        let pixel_height = 8 * 7;
        for pixel in pixels {

            let coords = termion::cursor::Goto((pixel_col * 2) + 1, pixel_row + 1);

            match pixel {
                0 => print!("{goto}{color}  ", goto = coords, color = termion::color::Bg(termion::color::White)),
                1 => print!("{goto}{color}  ", goto = coords, color = termion::color::Bg(termion::color::Blue)),
                2 => print!("{goto}{color}  ", goto = coords, color = termion::color::Bg(termion::color::LightBlue)),
                _ => print!("{goto}{color}  ", goto = coords, color = termion::color::Bg(termion::color::Black)),
            }
            
            pixel_col += 1;
            current_row_col += 1;
            if current_row_col > 7 {
                current_row_col = 0;
                pixel_col -= 8;
                pixel_row += 1;
                if pixel_row >= pixel_height {
                    pixel_row = 0;
                    pixel_col += 8;
                }
            }
        }
        println!("{reset}", reset = termion::style::Reset);
    }

    fn render_bitplanes(&self) {
        
        println!("{}", termion::clear::All);
        let pixel_height = 7 * 8;
        let mut pixel_row = 0;
        let mut pixel_col = 0;

        for byte in &self.bytes[..] {
            let coords = termion::cursor::Goto(pixel_col + 1, pixel_row + 1);
            let byte_string = format!("{:08b}", byte);
            let new_string: String = byte_string.chars().map(|x| match x {
                '0' => ' ',
                _ => '@',
            }).collect();
            print!("{}{}", coords, new_string);
            pixel_row += 1;
            if pixel_row >= pixel_height {
                pixel_row = 0;
                pixel_col += 8;
            }
        }
        println!("");
    }
}

fn main() {

    // Get the filename
    let args: Vec<String> = env::args().collect();

    let filename = match &args.get(1) {
        Some(filename) => filename.clone(),
        None => panic!("No filename specified!"),
    };

    println!("Filename: {}", &filename);

    let mut sprite_bytes = BitStream {
        bit_index: 0,
        byte_index: 0,
        last_two_bits: 0,
        bytes: Vec::new(),
    };
    sprite_bytes.load_bytes_from_file(&filename);
    println!("{:02X?}", sprite_bytes.bytes);

    // Read the first byte:
    // The first 4 bits are for the sprite width and the second 4 bits for the height

    let sprite_width: u8 = sprite_bytes.read_bits(4, false); // Read next 4 bits
    let sprite_height: u8 = sprite_bytes.read_bits(4, false); // Read next 4 bits

    // Width the width and height we can allocate the buffer
    let mut buffer = Buffer {
        bit_index: 0,
        byte_index: 0,
        width: 0,
        height: 0,
        vertical_offset: 0,
        horizontal_offset: 0,
        bytes: Vec::new(),
        bitplane_length: 0,
        row_index: 0,
    };
    buffer.allocate_space(sprite_width, sprite_height);

    println!("Sprite width: {}", buffer.width);
    println!("Sprite height: {}", buffer.height);
    println!("Vertical offset: {}", buffer.vertical_offset);
    println!("Horizontal offset: {}", buffer.horizontal_offset);

    // Primary buffer: this defines which bit buffer should be processed first
    let primary_buffer: u8 = sprite_bytes.read_bits(1, false); // Read next 1 bit
    println!("Primary buffer: {}", primary_buffer);


    // Initial packet type of the data
    // 0 means RLE packet and 1 means data packet
    let initial_packet: u8 = sprite_bytes.read_bits(1, false); // Read next 1 bit
    println!("Initial packet: {}", initial_packet);
    println!("Total bitplane length: {}", buffer.bitplane_length * 8);

    println!("Starting to decompress the first buffer!");
    buffer.decompress_to_bitplane(&mut sprite_bytes, initial_packet, primary_buffer == 0, true);
    // println!("{:02X?}", buffer.bytes);

    let encoding_mode: EncodingMode = {
        if sprite_bytes.current_bit() == 0 {
            sprite_bytes.next_bit();
            println!("Encoding mode 1");
            EncodingMode::Mode1
        } else {
            sprite_bytes.next_bit();
            match sprite_bytes.current_bit() {
                0 => {
                    println!("Encoding mode 2");
                    EncodingMode::Mode2
                },
                _ => {
                    println!("Encoding mode 3");
                    EncodingMode::Mode3
                },
            }
        }
    };
    sprite_bytes.next_bit();

    println!("Starting to decompress the second buffer!");
    let initial_packet: u8 = sprite_bytes.read_bits(1, false); // Read next 1 bit
    buffer.decompress_to_bitplane(&mut sprite_bytes, initial_packet, primary_buffer == 1, false);
    // println!("{:02X?}", buffer.bytes);

    // In mode 1 and 3, we have to delta-decode the buffer C
    // In any mode, we have to delta-decode the buffer B
    // In mode 2 and 3, xor buffer C against buffer B

    match encoding_mode {
        EncodingMode::Mode1 => {
            buffer.delta_decode(2);
            buffer.delta_decode(1);
        },
        EncodingMode::Mode2 => {
            buffer.delta_decode(2);
            buffer.xor_buffers(2, 1);
        },
        EncodingMode::Mode3 => {
            buffer.delta_decode(2);
            buffer.delta_decode(1);
            buffer.xor_buffers(2, 1);
        },
    }
    println!("Encoding result:");
    // println!("{:02X?}", buffer.bytes);

    // Now we need to copy the content from buffer B to A and from C to B,
    // but in the right order for the Gameboy to draw
    buffer.copy_bitplane(1, 0);
    buffer.copy_bitplane(2, 1);
    println!("Bitplane copy result:");
    // println!("{:02X?}", buffer.bytes);

    // Almost there!
    // Now we need to zipper the buffer A and B into buffer C and B going backwards
    buffer.zip_buffers();
    println!("Resulting zip:");
    // println!("{:02X?}", buffer.bytes);

    // And we can finally start rendering our sprite!!!

    // buffer.render_bitplanes();
    buffer.render();

}
