import sys
import re

def main():
    with open('src/server/debug_server.rs', 'r', encoding='utf-8') as f:
        content = f.read()

    # 1. Signature
    old_sig_1 = """    async fn handle_single_connection<S>(&mut self, stream: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,"""
    old_sig_2 = """    async fn handle_single_connection<S>(&mut self, stream: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + AsyncWrite + Unpin,"""
    new_sig = """    async fn handle_single_connection<S>(&mut self, stream: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,"""
    
    if old_sig_1 in content:
        content = content.replace(old_sig_1, new_sig)
    elif old_sig_2 in content:
        content = content.replace(old_sig_2, new_sig)
    else:
        print("Could not find handle_single_connection signature", file=sys.stderr)

    # 2. Variable setup
    old_setup = """        let mut authenticated = self.token.is_none();
        let mut handshake_done = false;
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| miette::miette!("Failed to read from stream: {}", e))?;
            if n == 0 {
                break;
            }"""
    
    new_setup = """        let mut authenticated = self.token.is_none();
        let mut handshake_done = false;
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = tokio::io::BufReader::new(reader);

        let (tx_in, mut rx_in) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (tx_out, mut rx_out) = tokio::sync::mpsc::unbounded_channel::<DebugMessage>();

        tokio::spawn(async move {
            while let Some(msg) = rx_out.recv().await {
                if crate::server::protocol::send_response(&mut writer, msg).await.is_err() {
                    break;
                }
            }
        });

        let tx_out_reader = tx_out.clone();
        let is_executing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let is_executing_reader = Arc::clone(&is_executing);

        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line).await.unwrap_or(0);
                if n == 0 { break; }

                if let Ok(msg) = DebugMessage::parse(line.trim_end()) {
                    if matches!(msg.request, Some(DebugRequest::Cancel)) {
                        let response = DebugMessage::response(msg.id, DebugResponse::CancelAck);
                        let _ = tx_out_reader.send(response);
                        if is_executing_reader.load(std::sync::atomic::Ordering::SeqCst) {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            eprintln!("Execution cancelled via request. Aborting with exit code 125.");
                            std::process::exit(125);
                        }
                        continue;
                    }
                }

                if tx_in.send(line.clone()).is_err() {
                    break;
                }
            }
        });

        // Helper closure to abstract away tx_out
        let mut send_msg = |msg: DebugMessage| -> Result<()> {
            tx_out.send(msg).map_err(|_| miette::miette!("Connection closed"))
        };

        loop {
            let line = match rx_in.recv().await {
                Some(l) => l,
                None => break,
            };
            is_executing.store(false, std::sync::atomic::Ordering::SeqCst);"""
            
    content = content.replace(old_setup, new_setup)

    # 3. Replace send_response everywhere
    content = re.sub(r'send_response\(\s*&mut\s*writer\s*,\s*response\s*\)\.await\?', r'send_msg(response)?', content)
    content = re.sub(r'send_response\(\s*&mut\s*writer\s*,\s*response\s*\)\.await', r'send_msg(response)', content)

    # 4. Inject `is_executing.store(true)` around executions
    # Execute without breakpoints
    exec_target = """execute_without_breakpoints(engine, &function, args)"""
    new_exec_target = """{ 
                                        is_executing.store(true, std::sync::atomic::Ordering::SeqCst);
                                        let r = execute_without_breakpoints(engine, &function, args);
                                        is_executing.store(false, std::sync::atomic::Ordering::SeqCst);
                                        r
                                    }"""
    content = content.replace(exec_target, new_exec_target)

    # Continue execution
    cont_target = """engine.execute_without_breakpoints(
                                &pending.function,
                                pending.args.as_deref(),
                            )"""
    new_cont_target = """{
                                is_executing.store(true, std::sync::atomic::Ordering::SeqCst);
                                let r = engine.execute_without_breakpoints(
                                    &pending.function,
                                    pending.args.as_deref(),
                                );
                                is_executing.store(false, std::sync::atomic::Ordering::SeqCst);
                                r
                            }"""
    content = content.replace(cont_target, new_cont_target)

    with open('src/server/debug_server.rs', 'w', encoding='utf-8') as f:
        f.write(content)
        
    print("Done refactoring debug_server.rs")

if __name__ == '__main__':
    main()
