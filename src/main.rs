//mod fs;

mod cache;
mod fs;

fn main() {
    let filesystem = fs::Fs::new(
        &cache::PathU8::from("."),
        fs::DEFAULT_MEM_LIMIT,
        fs::DEFAULT_ARCHIVE_LIMIT,
    );

    /*
    let server = http::new();

    server.on("path",(path){

        match fs.get(path){

            Some(nodeResult):{
                match nodeResult{
                    File(binary)=>{
                        writer.write(binary);
                    },
                    Dir(list)=>{

                        let first = true;
                        for i in list.iter() {
                            if !first {
                                writer.add("\n");
                            }
                            first = false;

                            writer.add(i.tobinary());
                        }
                    }
                }
            }

        }
    });
    */
}
