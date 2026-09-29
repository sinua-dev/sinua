// Just the Deno APIs the Supabase template uses, so `tsc` can check it
// alongside the others (Deno itself doesn't need this file).
declare const Deno: {
  serve(handler: (req: Request) => Response | Promise<Response>): unknown;
  env: { get(name: string): string | undefined; toObject(): Record<string, string> };
};
