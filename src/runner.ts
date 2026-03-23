import * as path from 'path';
import * as vscode from 'vscode';
import { resolveBinary } from './extension';

export function runBrainfuck(flags: string[] = []): void {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'brainfuck') {
        vscode.window.showErrorMessage('No Brainfuck file is open');
        return;
    }

    const filePath = editor.document.uri.fsPath;
    const interpreterPath = resolveBinary('interpreterPath', 'bf-interpreter');

    const terminal = vscode.window.createTerminal({
        name: `Brainfuck: ${path.basename(filePath)}`,
        shellPath: interpreterPath,
        shellArgs: [...flags, filePath],
    });
    terminal.show();
}
