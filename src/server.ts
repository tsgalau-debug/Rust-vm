import {
  AngularNodeAppEngine,
  createNodeRequestHandler,
  isMainModule,
  writeResponseToNodeResponse,
} from '@angular/ssr/node';
import express from 'express';
import type { Response as ExpressResponse } from 'express';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';

const browserDistFolder = join(import.meta.dirname, '../browser');
const aimtctlPath = resolve(process.cwd(), 'ai-microvm-tool-main/scripts/aimtctl.mjs');

// Gateway hardening: batas atas untuk proses aimtctl (bukan timeout WASM internal).
const AIMT_MAX_BUFFER = 4 * 1024 * 1024; // 4 MB > max_output_bytes(1MB) + wrapper JSON
const AIMT_GATEWAY_TIMEOUT_MS = 30_000;  // ceiling absolut gateway

function execAimtctl(
  sub: string,
  flags: string[],
  dashArgs: string[],
  timeoutMs: number | undefined,
  res: ExpressResponse,
) {
  const tail = dashArgs.length ? ['--', ...dashArgs] : [];
  const cmdArgs = [aimtctlPath, sub, ...flags, ...tail];

  const timeout = Math.min(
    (timeoutMs ?? AIMT_GATEWAY_TIMEOUT_MS) + 5_000,
    AIMT_GATEWAY_TIMEOUT_MS,
  );

  execFile(
    process.execPath,
    cmdArgs,
    { timeout, maxBuffer: AIMT_MAX_BUFFER },
    (err, stdout, stderr) => {
      try {
        return res.json(JSON.parse(stdout));
      } catch {
        const msg =
          err?.code === 'ETIMEDOUT'
            ? 'aimtctl timed out at gateway'
            : stderr || err?.message || `Failed to parse ${sub} output`;
        return res.status(500).json({ ok: false, error: msg });
      }
    },
  );
}

const app = express();
app.use(express.json());

const angularApp = new AngularNodeAppEngine();

/**
 * AI MicroVM API Endpoints
 */
app.get('/api/doctor', (_req, res) => {
  execAimtctl('doctor', [], [], undefined, res);
});

app.post('/api/explain', (req, res) => {
  const { profile = 'wasm-echo', policy = 'default', engine = 'auto', timeoutMs, args = [] } = req.body || {};
  const flags = ['--profile', String(profile), '--policy', String(policy), '--engine', String(engine)];
  if (timeoutMs) flags.push('--timeout-ms', String(timeoutMs));
  execAimtctl('explain', flags, args.map(String), timeoutMs, res);
});

app.post('/api/run', (req, res) => {
  const { profile = 'wasm-echo', policy = 'default', engine = 'auto', timeoutMs, args = [] } = req.body || {};
  const flags = ['--profile', String(profile), '--policy', String(policy), '--engine', String(engine), '--output', 'json'];
  if (timeoutMs) flags.push('--timeout-ms', String(timeoutMs));
  execAimtctl('run', flags, args.map(String), timeoutMs, res);
});

/**
 * Serve static files from /browser
 */
app.use(
  express.static(browserDistFolder, {
    maxAge: '1y',
    index: false,
    redirect: false,
  }),
);

/**
 * Handle all other requests by rendering the Angular application.
 */
app.use((req, res, next) => {
  angularApp
    .handle(req)
    .then((response) =>
      response ? writeResponseToNodeResponse(response, res) : next(),
    )
    .catch(next);
});

/**
 * Start the server if this module is the main entry point, or it is ran via PM2.
 * The server listens on the port defined by the `PORT` environment variable, or defaults to 4000.
 */
if (isMainModule(import.meta.url) || process.env['pm_id']) {
  const port = process.env['PORT'] || 4000;
  app.listen(port, (error) => {
    if (error) {
      throw error;
    }

    console.log(`Node Express server listening on http://localhost:${port}`);
  });
}

/**
 * Request handler used by the Angular CLI (for dev-server and during build) or Firebase Cloud Functions.
 */
export const reqHandler = createNodeRequestHandler(app);
