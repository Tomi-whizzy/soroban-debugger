import * as vscode from 'vscode';
import * as path from 'path';

export const OPEN_DOCS_COMMAND = 'soroban-debugger.openDocs';

interface DocItem extends vscode.QuickPickItem {
    file: string;
}

const DOC_ITEMS: DocItem[] = [
    {
        label: 'Source Map Health Diagnostics',
        description: 'Understand mapping quality and coverage',
        file: 'source-map-health.md'
    },
    {
        label: 'Debugger Architecture',
        description: 'Internal design of the debugger',
        file: 'architecture.md'
    },
    {
        label: 'Protocol Specification',
        description: 'Wire protocol for remote debugging',
        file: 'protocol.md'
    }
];

export async function openDocsCommand(context: vscode.ExtensionContext): Promise<void> {
    const selected = await vscode.window.showQuickPick(DOC_ITEMS, {
        placeHolder: 'Select a documentation page to open'
    });

    if (selected) {
        // Documentation is usually in the root 'docs' folder of the repository
        // But since this is an extension, we might want to open local docs or remote ones.
        // The user specifically asked for "project documentation".
        
        const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
        if (!workspaceFolder) {
            await vscode.window.showErrorMessage('Open a workspace to access project documentation.');
            return;
        }

        const docPath = vscode.Uri.joinPath(workspaceFolder.uri, 'docs', selected.file);
        
        try {
            const doc = await vscode.workspace.openTextDocument(docPath);
            await vscode.window.showTextDocument(doc, { preview: true });
        } catch (e) {
            await vscode.window.showErrorMessage(`Documentation file not found: ${selected.file}. Ensure you are in the soroban-debugger repository.`);
        }
    }
}
