# 1st gen Pokémon decompression algorithm in Rust

This is a mini-project I made just for fun and for practicing with bitwising and the Rust language.
The code is not the cleanest ever, but I have plans to improve it later.

The 1st generation Pokémon sprites are compressed with a very interesting algorithm very well explained [here](https://youtu.be/aF1Yw_wu2cM) and [here](https://youtu.be/ZI50XUeN6QE) by the Youtube channel [Retro Game Mechanics Explained](https://www.youtube.com/channel/UCwRqWnW5ZkVaP_lZF7caZ-g). He also has a [Github profile](https://github.com/Dotsarecool/) and he made a tool which was very useful for me during the development of this mini-project: [http://www.dotsarecool.com/rgme/tech/gen1decompress.html](http://www.dotsarecool.com/rgme/tech/gen1decompress.html).

I tried to implement the whole algorithm by myself (by only taking the videos as reference) but I struggled a bit with the delta-coding step, so I took some reference from [this project](https://github.com/xvillaneau/poke-sprite-python/) (Thanks btw!).

I haven't tested every single Pokémon yet (I wanna try if this works with glitched Pokémon). Feel free to test it with other pokémon!

## Usage
You will need to have Rust installed in order to compile the project.
```
cargo run compressed_pokemon_file
// or
./pokemon_sprite compressed_pokemon_file
```

## Where can I find a compressed Pokémon file? I wanna catch em' all!
I'm not sure if I can redistribute this files, but if you have a ROM of Pokémon Yellow (US) you can extract this (misteryous) Pokémon with this command!
```
dd if=pokemon-yellow-rom.gb of=who-is-that-pokemon.bin ibs=1 skip=183637 count=244
```
Or instead, you can use the Alex's tool to dump the sprites from a ROM file or for compressing your own images: [http://www.dotsarecool.com/rgme/tech/gen1decompress.html](http://www.dotsarecool.com/rgme/tech/gen1decompress.html)

# Contribution
As I mentioned, this is a project I made just for fun and practicing, but PR's and suggestions are welcome!

# Checklist 
- [ ] Test with more Pokémon (only Pokémons with mode 2 and 3 tested)
- [ ] Use (or create) a pixel engine to render the sprites instead of drawing them on the terminal
- [ ] Clean the code
- [ ] Parameters for changing the sprite size or color palette
