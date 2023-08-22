# bg3modmanager
The existing mod managers for Baldur's Gate 3 (e.g. https://github.com/LaughingLeader/BG3ModManager) all use [lslib](https://github.com/Norbyte/lslib), which is a great and robust library for working with LSPK pak files, but is written in C# and generally doesn't work great in Linux. As a result, I wrote a MINIMAL mod manager in Rust which should work in either Linux or Windows (although untested in Windows). This is very barebones and only supports what I needed to add a few mods. Fully open to pull requests and if anyone wants to use it as a library and provide a frontend I'm happy to refactor it as such.

# How to use
1. Download the source code and ensure you have Rust installed (either using your package manager or rustup)
2. Run `cargo run -- --mods-directory ~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian\ Studios/Baldur\'s\ Gate\ 3/Mods --player-profile-directory ~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian\ Studios/Baldur\'s\ Gate\ 3/PlayerProfiles/Public --add-mod <FilenameOfModInModsDirectory>`
3. Spot check your `modsettings.lsx` file to ensure it looks good and launch the game

# Limitations
Only adding a mod is supported. No removing or reodering. In the future I may add the ability to list the mods along with removal and reordering. If I do that I will probably also list all mods in the mods directory so you don't need to specify --add-mod.

# Acknowledgments
I inspected the [lslib](https://github.com/Norbyte/lslib) code quite a bit to figure out the file format for mods after getting the initial format from a [steam community post](https://steamcommunity.com/sharedfiles/filedetails/?id=2381865525). These were a huge help in getting this initial version working.
