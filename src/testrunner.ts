import * as path from 'path';
import * as vscode from 'vscode';

export async function runTests(context: vscode.ExtensionContext): Promise<void> {
    const uris = await vscode.window.showOpenDialog({
        canSelectMany: false,
        openLabel: 'Run Tests',
        filters: { 'Brainfuck Test Files': ['bft'] },
    });
    if (!uris || uris.length === 0) {
        return;
    }
    const bftPath = uris[0].fsPath;

    const config = vscode.workspace.getConfiguration('brainfuck');
    const configuredPath = config.get<string>('testRunnerPath');
    const runnerPath =
        configuredPath && configuredPath.trim() !== ''
            ? configuredPath
            : path.join(context.extensionPath, '..', 'target', 'debug', 'bf-testrunner');

    const channel = vscode.window.createOutputChannel('Brainfuck Tests');
    channel.show(true);
    channel.appendLine(`Running tests from: ${bftPath}`);
    channel.appendLine('');

    const { exec } = require('child_process');
    exec(
        `"${runnerPath}" "${bftPath}"`,
        (error: Error | null, stdout: string, stderr: string) => {
            if (stdout) channel.appendLine(stdout);
            if (stderr) channel.appendLine(stderr);

            const failed = stdout
                .split('\n')
                .filter(line => line.startsWith('FAIL'));

            if (failed.length > 0) {
                vscode.window
                    .showErrorMessage(
                        `${failed.length} test(s) failed`,
                        'Debug this failure'
                    )
                    .then(action => {
                        if (action === 'Debug this failure') {
                            launchTestDebug(failed[0], bftPath, context);
                        }
                    });
            } else if (!error) {
                vscode.window.showInformationMessage('All tests passed!');
            }
        }
    );
}

async function launchTestDebug(
    failLine: string,
    bftPath: string,
    context: vscode.ExtensionContext
): Promise<void> {
    // Parse the .bft file to extract the first failing test's details.
    // The fail line looks like: "FAIL  <name>  (<program>)"
    const match = failLine.match(/FAIL\s+(.+?)\s{2,}\((.+?)\)/);
    if (!match) return;

    const testName = match[1].trim();
    const programFile = match[2].trim();
    const bftDir = path.dirname(bftPath);
    const programPath = path.isAbsolute(programFile)
        ? programFile
        : path.join(bftDir, programFile);

    // Read .bft to get input and expectedOutput for this test.
    let input = '';
    let expectedOutput = '';
    try {
        const fs = require('fs');
        const content = fs.readFileSync(bftPath, 'utf8');
        // Simple extraction: look for the test block with matching name.
        const blocks = content.split('[[test]]').slice(1);
        for (const block of blocks) {
            if (block.includes(`name = "${testName}"`)) {
                const inMatch = block.match(/input\s*=\s*"([^"]*)"/);
                const outMatch = block.match(/expected_output\s*=\s*"([^"]*)"/);
                if (inMatch) input = inMatch[1];
                if (outMatch) expectedOutput = outMatch[1];
                break;
            }
        }
    } catch {}

    const config: vscode.DebugConfiguration = {
        type: 'brainfuck',
        request: 'testDebug',
        name: `Debug: ${testName}`,
        program: programPath,
        input,
        expectedOutput,
    };

    const folder = vscode.workspace.workspaceFolders?.[0];
    await vscode.debug.startDebugging(folder, config);
}
