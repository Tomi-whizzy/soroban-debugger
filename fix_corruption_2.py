import os
import re

def fix_security_rs():
    with open('src/analyzer/security.rs', 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Add call_depth: Some(0) to DynamicTraceEvent literals that have function: ...
    # and don't already have call_depth.
    
    def repl(match):
        inner = match.group(1)
        if 'call_depth' in inner:
            return match.group(0)
        return inner + '\n                call_depth: Some(0),'

    pattern = r'(DynamicTraceEvent\s*\{[^}]*?function:\s*[^,}]*?,)'
    new_content = re.sub(pattern, repl, content)
    
    # For storage-only ones, they might not have function:
    pattern2 = r'(DynamicTraceEvent\s*\{[^}]*?kind:\s*DynamicTraceEventKind::StorageRead,[^}]*?message:\s*[^,}]*?,)'
    def repl2(match):
        inner = match.group(1)
        if 'call_depth' in inner:
            return match.group(0)
        return inner + '\n                call_depth: None,'
    new_content = re.sub(pattern2, repl2, new_content)

    with open('src/analyzer/security.rs', 'w', encoding='utf-8') as f:
        f.write(new_content)

def fix_remote_client_rs():
    with open('src/client/remote_client.rs', 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Fix the return Err(e) to return Err(e.into()) or just use ?
    # In cancel() method:
    # 405:             Err(e) => return Err(e),
    content = content.replace('Err(DebuggerError::NetworkError(ref msg)) if msg.contains("No response") => {', 
                              'Err(e) if e.to_string().contains("No response") => {')
    # This also fixes the type because e is already a Report in send_request return.
    
    # Wait, send_request returns Result<DebugResponse>.
    # Result is crate::Result which is miette::Result.
    
    # Let's just fix the return statement in cancel()
    content = content.replace('return Err(e),', 'return Err(e),') # No-op just to be sure
    # Actually if e is miette::Report, Err(e) IS Result<T, Report>.
    # So why did it fail?
    # Ah! DebuggerError is NOT Report.
    # In remote_client.rs, send_request returns Result<DebugResponse>.
    # If it fails, it returns Err(Report).
    
    # Let's check send_request return type.
    # 418:     fn send_request(&mut self, request: DebugRequest) -> Result<DebugResponse> {
    
    # If cancel() calls send_request and matches on Err(e), then e IS Report.
    # So return Err(e) SHOULD work.
    
    # Wait, the error message from the log was:
    # ^^^^^^^^^^ expected `Report`, found t::Resul 
    # `DebuggerError`p,
    
    # Maybe it was the DISCONNECT return?
    # 413:             _ => Err(DebuggerError::ExecutionError("Unexpected response to Cancel".to_string()).into()),
    
    # If I use .into() on DebuggerError, it should become Report.
    
    with open('src/client/remote_client.rs', 'w', encoding='utf-8') as f:
        f.write(content)

if __name__ == '__main__':
    fix_security_rs()
    fix_remote_client_rs()
