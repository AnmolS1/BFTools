import * as vscode from 'vscode';
import { resolveBinary } from './extension';

export class BrainfuckDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
    createDebugAdapterDescriptor(
        _session: vscode.DebugSession,
        _executable: vscode.DebugAdapterExecutable | undefined
    ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
        const dapPath = resolveBinary('dapServerPath', 'bf-dap');
        return new vscode.DebugAdapterExecutable(dapPath);
    }
}

export class BrainfuckConfigurationProvider implements vscode.DebugConfigurationProvider {
    resolveDebugConfiguration(
        _folder: vscode.WorkspaceFolder | undefined,
        config: vscode.DebugConfiguration,
        _token?: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.DebugConfiguration> {
        if (!config.type) {
            config.type = 'brainfuck';
        }
        if (!config.request) {
            config.request = 'launch';
        }
        if (!config.name) {
            config.name = 'Debug Brainfuck';
        }
        if (!config.program) {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'brainfuck') {
                config.program = editor.document.fileName;
            } else {
                vscode.window.showErrorMessage('No active Brainfuck file to debug');
                return undefined;
            }
        }
        return config;
    }
}
