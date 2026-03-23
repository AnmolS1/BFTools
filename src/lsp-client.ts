import * as path from 'path';
import * as vscode from 'vscode';
import type { LanguageClient as LanguageClientType } from 'vscode-languageclient/node';

let client: LanguageClientType | undefined;

export function createLspClient(context: vscode.ExtensionContext): LanguageClientType {
    const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

    const config = vscode.workspace.getConfiguration('brainfuck');
    let serverPath = config.get<string>('lspServerPath', '');
    if (!serverPath) {
        serverPath = path.join(context.extensionPath, '..', 'target', 'debug', 'bf-lsp');
    }

    const serverOptions = {
        run: { command: serverPath, transport: TransportKind.stdio },
        debug: { command: serverPath, transport: TransportKind.stdio },
    };

    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'brainfuck' }],
        outputChannel: vscode.window.createOutputChannel('Brainfuck LSP'),
    };

    client = new LanguageClient(
        'brainfuck-lsp',
        'Brainfuck LSP',
        serverOptions,
        clientOptions,
    );

    context.subscriptions.push({ dispose: () => stopLspClient() });
    return client!;
}

export async function startLspClient(): Promise<void> {
    if (client) {
        await client.start();
    }
}

export async function stopLspClient(): Promise<void> {
    if (client) {
        await client.stop();
        client = undefined;
    }
}
