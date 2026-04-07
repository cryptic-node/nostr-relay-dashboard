<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    
</head>
<body class="bg-zinc-950 text-zinc-100">
    <div class="max-w-4xl mx-auto p-8">
        <!-- Header -->
        <div class="flex items-center justify-between mb-10 border-b border-zinc-800 pb-8">
            <div class="flex items-center gap-x-4">
                <i class="fa-solid fa-bolt text-purple-400 text-5xl"></i>
                <div>
                    <h1 class="text-4xl font-semibold">Nostr Relay Dashboard</h1>
                    <p class="text-purple-400 text-2xl font-medium">v1.0.1 — Stable Production Release</p>
                </div>
            </div>
            <div class="flex items-center gap-x-2 text-sm">
                <i class="fa-solid fa-circle-check text-emerald-400"></i>
                <span class="font-medium status-green">Live • Fully Functional • Perfect</span>
            </div>
        </div>

        <div class="prose prose-invert max-w-none">
            <p class="text-xl text-zinc-300 leading-relaxed mb-10">
                Self-hosted Nostr event aggregator and personal backup dashboard. 
                Pulls real Kind=1 text notes (signed by your monitored npubs) from configurable upstream relays, 
                stores them locally, and gives you a clean, fast, mobile-friendly dark UI to browse, backup, and restore everything.
            </p>

            <div class="bg-emerald-950 border border-emerald-900 rounded-3xl px-6 py-4 mb-10 flex items-center gap-x-3 text-emerald-300">
                <i class="fa-solid fa-link"></i>
                <strong>Live URL (main branch):</strong> 
                <a href="http://159.89.49.4:8080" target="_blank" class="underline hover:text-emerald-400">http://159.89.49.4:8080</a>
            </div>

            <h2 class="text-2xl font-semibold mb-6 flex items-center gap-x-3">
                <i class="fa-solid fa-circle-check text-emerald-400"></i>
                Current Features (v1.0.1)
            </h2>

            <ul class="space-y-6 mb-12">
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">📡</div>
                    <div>
                        <strong class="block">Clean three-panel layout</strong>
                        <span class="text-zinc-400">Left: relays • Middle: npubs (purple highlight) • Right: recent notes with capped height + clean scrollbar</span>
                    </div>
                </li>
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">🔥</div>
                    <div>
                        <strong class="block">Real Nostr pulling</strong>
                        <span class="text-zinc-400">Only Kind=1 text notes signed by monitored npubs using official nostr-sdk. Preloaded relays + Umbrel private relay included.</span>
                    </div>
                </li>
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">🔄</div>
                    <div>
                        <strong class="block">Sync options</strong>
                        <span class="text-zinc-400">Manual “Sync Now” button + automatic nightly sync</span>
                    </div>
                </li>
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">💾</div>
                    <div>
                        <strong class="block">Full NDJSON backup &amp; restore</strong>
                        <span class="text-zinc-400">One-click backup and restore with validation and import count</span>
                    </div>
                </li>
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">📜</div>
                    <div>
                        <strong class="block">Verbose logging + controls</strong>
                        <span class="text-zinc-400">Download Logs, Restart Server, green status messages — no confirmation popups</span>
                    </div>
                </li>
                <li class="flex gap-x-4">
                    <div class="w-8 h-8 bg-zinc-900 rounded-2xl flex items-center justify-center text-purple-400 flex-shrink-0">📱</div>
                    <div>
                        <strong class="block">Polished UX</strong>
                        <span class="text-zinc-400">Safe UTF-8 truncation, mobile-friendly dark UI, emerald/green accents, exact locked layout</span>
                    </div>
                </li>
            </ul>

            <h2 class="text-2xl font-semibold mb-6">Bottom Control Bar</h2>
            <div class="flex flex-wrap gap-3 bg-zinc-900 rounded-3xl p-6 mb-12">
                <div class="bg-emerald-500 text-white px-6 py-3 rounded-2xl flex-1 text-center text-sm font-medium">Sync Now</div>
                <div class="bg-violet-500 text-white px-6 py-3 rounded-2xl flex-1 text-center text-sm font-medium">Backup (NDJSON)</div>
                <div class="bg-amber-500 text-white px-6 py-3 rounded-2xl flex-1 text-center text-sm font-medium">Restore</div>
                <div class="bg-sky-500 text-white px-6 py-3 rounded-2xl flex-1 text-center text-sm font-medium">Download Logs</div>
                <div class="bg-red-500 text-white px-6 py-3 rounded-2xl flex-1 text-center text-sm font-medium">Restart Server</div>
            </div>

            <h2 class="text-2xl font-semibold mb-6">Quick Start (Droplet)</h2>
            <div class="bg-zinc-900 rounded-3xl p-8 font-mono text-emerald-300 text-sm leading-relaxed mb-12">
                git clone https://github.com/cryptic-node/nostr-relay-dashboard.git<br>
                cd nostr-relay-dashboard<br>
                git checkout main<br>
                cargo build --release<br>
                tmux new-session -d -s nostr-relay-dashboard './target/release/nostr-relay-dashboard'<br><br>
                Open → <a href="http://159.89.49.4:8080" class="text-emerald-400 underline">http://159.89.49.4:8080</a>
            </div>

            <div class="text-center py-8 border border-dashed border-zinc-700 rounded-3xl">
                <p class="text-zinc-400">This is the perfect, stable v1.0.1 production version.</p>
                <p class="text-zinc-400 mt-2">Main branch = v1.0.1 (frozen) • develop branch = v1.0.2 (future work)</p>
            </div>

            <div class="mt-16 text-center text-xs text-zinc-500">
                Made for cryptic-node • April 2026 • All preferences locked in
            </div>
        </div>
    </div>
</body>
</html>
