import * as path from 'path';
import * as vscode from 'vscode';

export function runBrainfuck(
    context: vscode.ExtensionContext,
    flags: string[] = [],
): void {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'brainfuck') {
        vscode.window.showErrorMessage('No Brainfuck file is open');
        return;
    }

    const filePath = editor.document.uri.fsPath;
    const config = vscode.workspace.getConfiguration('brainfuck');
    let interpreterPath = config.get<string>('interpreterPath', '');
    if (!interpreterPath) {
        interpreterPath = path.join(context.extensionPath, '..', 'target', 'debug', 'bf-interpreter');
    }

    const terminal = vscode.window.createTerminal({
        name: `Brainfuck: ${path.basename(filePath)}`,
        shellPath: interpreterPath,
        shellArgs: [...flags, filePath],
    });
    terminal.show();
}
