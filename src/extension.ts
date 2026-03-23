import * as path from 'path';
import * as vscode from 'vscode';
import { createLspClient, startLspClient, stopLspClient } from './lsp-client';
import { runBrainfuck } from './runner';
import { BrainfuckDebugAdapterFactory, BrainfuckConfigurationProvider } from './dap-client';
import { runTests } from './testrunner';

let outputChannel: vscode.OutputChannel;
let extensionPath: string = '';

/**
 * Resolve the path to a Brainfuck tool binary.
 *
 * Search order:
 *  1. User-configured path in settings (e.g. brainfuck.lspServerPath)
 *  2. <workspace-root>/target/release/<binaryName>
 *  3. <workspace-root>/target/debug/<binaryName>
 *  4. <extensionPath>/bin/<binaryName> (bundled in VSIX)
 *  5. Bare binary name (relies on PATH)
 */
export function resolveBinary(
    settingKey: string,
    binaryName: string,
): string {
    const config = vscode.workspace.getConfiguration('brainfuck');
    const configured = config.get<string>(settingKey, '');
    if (configured && configured.trim() !== '') {
        return configured.trim();
    }

    const fs: typeof import('fs') = require('fs');

    // Check workspace target directories (for development)
    const wsRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (wsRoot) {
        const release = path.join(wsRoot, 'target', 'release', binaryName);
        if (fs.existsSync(release)) { return release; }
        const debug = path.join(wsRoot, 'target', 'debug', binaryName);
        if (fs.existsSync(debug)) { return debug; }
    }

    // Check bundled binaries shipped inside the VSIX
    if (extensionPath) {
        const bundled = path.join(extensionPath, 'bin', binaryName);
        if (fs.existsSync(bundled)) { return bundled; }
        const bundledExe = bundled + '.exe';
        if (fs.existsSync(bundledExe)) { return bundledExe; }
    }

    // Fall back to bare name and rely on PATH
    return binaryName;
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    extensionPath = context.extensionPath;
    outputChannel = vscode.window.createOutputChannel('Brainfuck');
    outputChannel.appendLine('Brainfuck extension activated');

    const commands: [string, (...args: any[]) => any][] = [
        ['brainfuck.run', () => runBrainfuck()],
        ['brainfuck.runJIT', () => runBrainfuck(['--jit'])],
        ['brainfuck.runAutoJIT', () => runBrainfuck(['--auto-jit'])],
        ['brainfuck.format', () => vscode.commands.executeCommand('editor.action.formatDocument')],
        ['brainfuck.precompile', () => precompile()],
        ['brainfuck.runTests', () => runTests(context)],
    ];

    for (const [id, handler] of commands) {
        context.subscriptions.push(vscode.commands.registerCommand(id, handler));
    }

    context.subscriptions.push(outputChannel);

    // Register DAP debug adapter
    const factory = new BrainfuckDebugAdapterFactory();
    const provider = new BrainfuckConfigurationProvider();
    context.subscriptions.push(
        vscode.debug.registerDebugAdapterDescriptorFactory('brainfuck', factory),
        vscode.debug.registerDebugConfigurationProvider('brainfuck', provider),
    );

    try {
        createLspClient();
        await startLspClient();
    } catch (err) {
        outputChannel.appendLine(`LSP client failed to start: ${err}`);
    }
}

async function precompile(): Promise<void> {
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
    if (!outputUri) { return; }

    const precompilerPath = resolveBinary('precompilerPath', 'bf-precompiler');
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
