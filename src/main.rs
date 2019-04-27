
mod fs;

mod cache;


fn main() {


    let fs= fs::Fs::new(cache::PathU8::new(), fs::DEFAULT_LIMIT);
 //
 //

    //let _tree = cache::CacheFsTree::new(256 * 1024 * 1024);

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
