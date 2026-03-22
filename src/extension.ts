import * as path from 'path';
import * as vscode from 'vscode';
import { createLspClient, startLspClient, stopLspClient } from './lsp-client';
import { runBrainfuck } from './runner';
import { BrainfuckDebugAdapterFactory, BrainfuckConfigurationProvider } from './dap-client';
import { runTests } from './testrunner';

let outputChannel: vscode.OutputChannel;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    outputChannel = vscode.window.createOutputChannel('Brainfuck');
    outputChannel.appendLine('Brainfuck extension activated');

    const commands: [string, (...args: any[]) => any][] = [
        ['brainfuck.run', () => runBrainfuck(context)],
        ['brainfuck.runJIT', () => runBrainfuck(context, ['--jit'])],
        ['brainfuck.runAutoJIT', () => runBrainfuck(context, ['--auto-jit'])],
        ['brainfuck.format', () => vscode.commands.executeCommand('editor.action.formatDocument')],
        ['brainfuck.precompile', () => precompile(context)],
        ['brainfuck.runTests', () => runTests(context)],
    ];

    for (const [id, handler] of commands) {
        context.subscriptions.push(vscode.commands.registerCommand(id, handler));
    }

    context.subscriptions.push(outputChannel);

    // Register DAP debug adapter
    const factory = new BrainfuckDebugAdapterFactory(context);
    const provider = new BrainfuckConfigurationProvider();
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory('brainfuck', factory),
        vscode.debug.registerDebugConfigurationProvider('brainfuck', provider),
    );

    try {
        createLspClient(context);
        await startLspClient();
    } catch (err) {
        outputChannel.appendLine(`LSP client failed to start: ${err}`);
    }
}

async function precompile(context: vscode.ExtensionContext): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'brainfuck') {
        vscode.window.showErrorMessage('No active Brainfuck file to precompile');
        return;
    }
    const sourcePath = editor.document.fileName;

    const outputUri = await vscode.window.showSaveDialog({
        defaultUri: vscode.Uri.file(sourcePath.replace(/\.bf$/, '')),
        saveLabel: 'Save Executable As',
    });
    if (!outputUri) return;

    const config = vscode.workspace.getConfiguration('brainfuck');
    const configuredPath = config.get<string>('precompilerPath');
    const precompilerPath =
        configuredPath && configuredPath.trim() !== ''
            ? configuredPath
            : path.join(context.extensionPath, '..', 'target', 'debug', 'bf-precompiler');

    const terminal = vscode.window.createTerminal('Brainfuck Precompiler');
    terminal.show();
    terminal.sendText(
        `"${precompilerPath}" "${sourcePath}" -o "${outputUri.fsPath}" && echo "Done: ${outputUri.fsPath}"`
    );
}

export async function deactivate(): Promise<void> {
    if (outputChannel) {
        outputChannel.dispose();
    }
    await stopLspClient();
}
