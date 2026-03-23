import * as vscode from 'vscode';
import type { LanguageClient as LanguageClientType } from 'vscode-languageclient/node';
import { resolveBinary } from './extension';

let client: LanguageClientType | undefined;

export function createLspClient(): LanguageClientType {
    const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

    const serverPath = resolveBinary('lspServerPath', 'bf-lsp');

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

    return client!;
}

export async function startLspClient(): Promise<void> {
    if (client) {
        await client.start();
    }
}

export async function stopLspClient(): Promise<void> {
    if (client) {
        try {
            await client.stop();
        } catch {
            // Client may be in startFailed state — safe to ignore
        }
        client = undefined;
    }
}
