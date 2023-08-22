use std::{fs::File, os::linux::raw, str::{self, from_utf8}, mem::{size_of, self}};

use clap::Parser;
use xmltree::{Element, XMLNode};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory where mods are stored. Typically ~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3/Mods
    #[arg(short, long, default_value = "~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3/Mods")]
    mods_directory: String,

    /// The directory where modsettings.lsx is stored. Typically ~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3/PlayerProfiles/Public
    #[arg(short, long, default_value = "~/.steam/steam/steamapps/compatdata/1086940/pfx/drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3/PlayerProfiles/Public")]
    player_profile_directory: String,

    /// Filename of a mod already located in the mods directory to activate.
    #[arg(short, long)]
    add_mod: String,
}

#[repr(C, packed(1))]
struct FileEntry {
    name: [u8; 256],
    offset_in_file_low: u32,
    offest_in_file_high: u16,
    archive: u8,
    flags: u8,
    size_on_disk: u32,
    uncompressed_size: u32,
}

fn main() {
    let args = Args::parse();

    let mod_file = std::fs::read(args.mods_directory + "/" + &args.add_mod).unwrap();
    // Confirm this is an LSPK
    assert!(mod_file[0..4] == *"LSPK".as_bytes());
    let header_version = u32::from_le_bytes(mod_file[4..8].try_into().expect("Missing version"));
    // We only support version 18
    assert!(header_version == 18);
    // Table offset is 12 bytes in
    let file_table_offset = u64::from_le_bytes(mod_file[8..16].try_into().expect("Not enough bytes")) as usize;
    println!("Table offset: {}", file_table_offset);
    let file_table_size = u32::from_le_bytes(mod_file[16..20].try_into().expect("Not enough bytes"));
    // TODO: Do we need these parts?
    // Flags: byte
    // Priority: byte
    // MD5: 16 bytes
    // PartsCount: u16

    let file_table = &mod_file[file_table_offset..(file_table_offset + file_table_size as usize)];

    let file_count = i32::from_le_bytes(file_table[0..4].try_into().expect("Not enough bytes"));
    println!("File count: {}", file_count);
    let compressed_size = i32::from_le_bytes(file_table[4..8].try_into().expect("Not enough bytes"));
    let compressed_file_table = &file_table[8..(8 + compressed_size as usize)];
    let mut buffer = Vec::new();
    buffer.resize(size_of::<FileEntry>() * file_count as usize, 0);
    lz4_flex::decompress_into(compressed_file_table, buffer.as_mut_slice()).expect("Failed to decompress file table");

    for i in 0..file_count {
        let file: &FileEntry = unsafe { mem::transmute(buffer[(i as usize * size_of::<FileEntry>())..].as_ptr()) };
        let file_name = from_utf8(&file.name).unwrap();
        println!("File name: {}", file_name);
        println!("Flags: {}", file.flags);
        let compression_method = file.flags & 0x0F;
        if compression_method == 0x01 {
            println!("Compressed with Zlib!");
        } else if compression_method == 0x02 {
            println!("Compressed with LZ4!");
        } else if compression_method != 0x00 {
            panic!("Unexpected compression method {}", compression_method);
        } else {
            println!("Uncompressed!");
        }
        let file_start = ((file.offest_in_file_high as u64) << 32) + file.offset_in_file_low as u64;
        println!("Offset: {}", file_start);

        if file_name.contains("/meta.lsx") {
            // I'm assuming uncompressed
            let file_start = ((file.offest_in_file_high as u64) << 32) + file.offset_in_file_low as u64;
            let mut file_contents = &mod_file[(file_start as usize)..((file_start + file.size_on_disk as u64) as usize)];
            let mut buffer = Vec::new();
            if compression_method == 0x02 /* LZ4 */ {
                buffer.resize(file.uncompressed_size as usize, 0);
                lz4_flex::decompress_into(file_contents, buffer.as_mut_slice()).expect("Failed to decompress file");
                file_contents = &buffer;
            }
            println!("Contents: {}", from_utf8(file_contents).unwrap());

            // FIXME: Now parse the XML, add the new bits to the mods_children from above, and profit
            let mod_config = xmltree::Element::parse(from_utf8(file_contents).unwrap().as_bytes()).unwrap();
            let mod_config = get_mod_config(&mod_config);
            // The MD5 must be the sum of the entire pak file.
            let md5_sum = format!("{:x}", md5::compute(&mod_file));
            println!("MD5: {}", md5_sum);

            let mut module_node = xmltree::Element::new("node");
            module_node.attributes.insert("id".to_owned(), "ModuleShortDesc".to_owned());
            for child in &mod_config.children {
                let child = child.as_element();
                if child.is_none() {
                    continue;
                }
                let child = child.unwrap();
                if child.name != "attribute" {
                    continue;
                }
                let id = child.attributes.iter().find(|a| a.0 == "id");
                if id.is_none() {
                    continue;
                }
                let id = id.unwrap();

                if id.1 == "Folder" || id.1 == "Name" || id.1 == "UUID" || id.1 == "Version" || id.1 == "Version64" {
                    module_node.children.push(XMLNode::Element(child.clone()));
                }
            }

            let mut md5_node = xmltree::Element::new("attribute");
            md5_node.attributes.insert("id".to_owned(), "MD5".to_owned());
            md5_node.attributes.insert("type".to_owned(), "LSString".to_owned());
            md5_node.attributes.insert("value".to_owned(), md5_sum);

            let raw_xml = std::fs::read_to_string(args.player_profile_directory.clone() + "/modsettings.lsx").unwrap();
            let mut xml = xmltree::Element::parse(raw_xml.as_bytes()).unwrap();

            {
                let mods_children = get_mods_children(&mut xml);
                mods_children.children.push(XMLNode::Element(module_node));
            }
            xml.write(std::fs::File::options().write(true).truncate(true).open(args.player_profile_directory.clone() + "/modsettings.lsx").unwrap()).expect("Failed to write modsettings.lsx");
            return;
        }
    }
}

fn get_mods_children(xml: &mut Element) -> &mut Element {
    if xml.children.iter()
        .find(|n| n.as_element()
            .map(|e| e.name == "region" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "ModuleSettings"))
            .unwrap_or(false)).is_none() {
        let mut el = Element::new("region");
        el.attributes.insert("id".to_owned(), "ModuleSettings".to_owned());
        xml.children.push(XMLNode::Element(el));
    }
    let region_node = xml.children.iter_mut()
    .find(|n| n.as_element()
        .map(|e| e.name == "region" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "ModuleSettings"))
        .unwrap_or(false)).unwrap().as_mut_element().unwrap();

    if !region_node.children.iter().any(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "root")).unwrap_or(false)) {
        let mut el = Element::new("node");
        el.attributes.insert("id".to_owned(), "root".to_owned());
        region_node.children.push(XMLNode::Element(el));
    }
    let root_node = region_node.children.iter_mut().find(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "root")).unwrap_or(false)).unwrap().as_mut_element().unwrap();

    if root_node.get_child("children").is_none() {
        root_node.children.push(XMLNode::Element(Element::new("children")));
    }
    let children_node = root_node.get_mut_child("children").unwrap();

    if !children_node.children.iter().any(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "Mods")).unwrap_or(false)) {
        let mut el = Element::new("node");
        el.attributes.insert("id".to_owned(), "Mods".to_owned());
        children_node.children.push(XMLNode::Element(el));
    }
    let mods_node = children_node.children.iter_mut().find(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "Mods")).unwrap_or(false)).unwrap().as_mut_element().unwrap();

    if mods_node.get_child("children").is_none() {
        mods_node.children.push(XMLNode::Element(Element::new("children")));
    }
    mods_node.get_mut_child("children").unwrap()

}

fn get_mod_config(xml: &Element) -> &Element {
    let save_node = if xml.name == "save" { xml } else { xml.get_child("save").unwrap() };

    let region_node = save_node.children.iter()
    .find(|n| n.as_element()
        .map(|e| e.name == "region" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "Config"))
        .unwrap_or(false)).unwrap().as_element().unwrap();

    let root_node = region_node.children.iter().find(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "root")).unwrap_or(false)).unwrap().as_element().unwrap();

    let children_node = root_node.get_child("children").unwrap();

    let mod_info_node = children_node.children.iter().find(|n| n.as_element().map(|e| e.name == "node" && e.attributes.iter().any(|a| a.0 == "id" && a.1 == "ModuleInfo")).unwrap_or(false)).unwrap().as_element().unwrap();

    mod_info_node
}