
//mod fs;

mod cache;


fn main() {


 //   let fs= Fs::new();
 //
 //

    let tree = cache::CacheFsTree::new();

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
