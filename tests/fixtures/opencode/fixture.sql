-- Scrubbed fixture for the OpenCode adapter tests: the tables and columns the
-- adapter reads (plus the NOT NULL ones), shaped like a real opencode.db.
-- Tests build a temporary database from it with rusqlite (rollback-journal
-- mode, so a read-only open never creates side files).

CREATE TABLE project (
  id TEXT PRIMARY KEY,
  worktree TEXT NOT NULL,
  name TEXT,
  time_created INTEGER NOT NULL,
  time_updated INTEGER NOT NULL,
  sandboxes TEXT NOT NULL
);
CREATE TABLE session_v2 (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  parent_id TEXT,
  slug TEXT NOT NULL,
  directory TEXT NOT NULL,
  title TEXT,
  version TEXT NOT NULL,
  agent TEXT,
  model TEXT,
  cost REAL DEFAULT 0 NOT NULL,
  tokens_input INTEGER DEFAULT 0 NOT NULL,
  tokens_output INTEGER DEFAULT 0 NOT NULL,
  time_created INTEGER NOT NULL,
  time_updated INTEGER NOT NULL,
  time_idle INTEGER
);
CREATE INDEX session_v2_parent_idx ON session_v2 (parent_id);
CREATE TABLE session_message (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  type TEXT NOT NULL,
  seq INTEGER NOT NULL,
  time_created INTEGER NOT NULL,
  time_updated INTEGER NOT NULL,
  data TEXT NOT NULL
);
CREATE UNIQUE INDEX session_message_session_seq_idx ON session_message (session_id, seq);
CREATE TABLE migration (id TEXT PRIMARY KEY, time_completed INTEGER NOT NULL);

INSERT INTO migration VALUES ('20261029002625_session_part', 1793000000000);
INSERT INTO migration VALUES ('20261029073502_session_time_busy', 1793000000001);
INSERT INTO migration VALUES ('20261029165540_schedule_checks', 1793000000002);

INSERT INTO project VALUES ('prj_alpha', '/home/user/alpha', 'Alpha App', 1790812800000, 1790812800000, '[]');
INSERT INTO project VALUES ('prj_beta', '/home/user/work/beta-tool', NULL, 1790812800000, 1790812800000, '[]');
INSERT INTO project VALUES ('prj_empty', '/home/user/unused', NULL, 1790812800000, 1790812800000, '[]');

INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_main', 'prj_alpha', NULL, 'ses_main', '/home/user/alpha', 'Add the settings screen', '1.0.0', NULL, '{"id":"acme/coder-1","providerID":"acme"}', 1793178000000, 1793179800000, 1793179800000);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_01', 'ses_main', 'synthetic', 1, 1793178000500, 1793178000500, '{"time":{"created":1793178000500},"text":"<system-reminder>Note: The user opened the file src/settings.rs.</system-reminder>\n"}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_02', 'ses_main', 'user', 2, 1793178001000, 1793178001000, '{"time":{"created":1793178001000},"text":"Add a settings screen with a dark mode toggle.","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_03', 'ses_main', 'system', 3, 1793178002000, 1793178002000, '{"time":{"created":1793178002000},"text":"New skills are available:\n  <skill><id>review</id></skill>","description":"Instructions updated: core/skill-guidance","metadata":{"notice":"instructions"}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_04', 'ses_main', 'assistant', 4, 1793178003000, 1793178003000, '{"time":{"created":1793178003000,"streamed":1793178009000,"completed":1793178009000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"high"},"content":[{"type":"reasoning","text":"I should look at the existing screens first.","time":{"created":1793178003000}},{"type":"text","text":"\n\n"},{"type":"text","text":"Let me look at the repo layout."},{"type":"tool","id":"call-read-1","name":"read","executed":false,"state":{"status":"completed","input":{"path":"/home/user/alpha/src/main.rs"},"content":[{"type":"text","text":"Read file /home/user/alpha/src/main.rs\n1: fn main() {}\n"}]},"time":{"created":1793178004000,"ran":1793178004000,"completed":1793178005000}},{"type":"tool","id":"call-shell-1","name":"shell","executed":false,"state":{"status":"completed","input":{"command":"ls src","description":"List the sources"},"content":[{"type":"text","text":"main.rs\nsettings.rs\n"}]},"time":{"created":1793178006000,"ran":1793178006000,"completed":1793178008000}}],"finish":"stop","cost":0.01,"tokens":{"input":1000,"output":100,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_05', 'ses_main', 'assistant', 5, 1793178010000, 1793178010000, '{"time":{"created":1793178010000,"streamed":1793178020000,"completed":1793178020000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"tool","id":"call-shell-2","name":"shell","executed":false,"state":{"status":"error","input":{"command":"cargo test"},"content":[{"type":"text","text":"error: test failed\n"}],"error":{"type":"tool.failed","message":"exit code 101"}},"time":{"created":1793178011000,"ran":1793178011000,"completed":1793178015000}},{"type":"tool","id":"call-write-1","name":"write","executed":false,"state":{"status":"error","input":{"path":"src/scratch.txt","content":"x\n"},"error":{"type":"permission.rejected","message":"Permission denied: edit"}},"time":{"created":1793178016000,"ran":1793178016000,"completed":1793178017000}}],"finish":"stop","cost":0.02,"tokens":{"input":2000,"output":200,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_06', 'ses_main', 'idle', 6, 1793178021000, 1793178021000, '{"time":{"created":1793178021000},"outcome":"succeeded"}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_07', 'ses_main', 'user', 7, 1793178300000, 1793178300000, '{"time":{"created":1793178300000},"text":"Now delegate the review.","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_08', 'ses_main', 'assistant', 8, 1793178301000, 1793178301000, '{"time":{"created":1793178301000,"streamed":1793178330000,"completed":1793178330000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"text","text":"Asking the explorer and the reviewer."},{"type":"tool","id":"call-sub-1","name":"subagent","executed":false,"state":{"status":"completed","input":{"agent":"explore","description":"Explore the repo","prompt":"Look around the repository"},"content":[{"type":"text","text":"Two crates: a lib and a binary."}],"metadata":{"sessionID":"ses_child1","status":"completed","truncated":false}},"time":{"created":1793178302000,"ran":1793178302000,"completed":1793178310000}},{"type":"tool","id":"call-sub-2","name":"subagent","executed":false,"state":{"status":"completed","input":{"agent":"review","description":"Review the code","prompt":"Review the change"},"content":[{"type":"text","text":"Looks good to me."}]},"time":{"created":1793178311000,"ran":1793178311000,"completed":1793178329000}}],"finish":"stop","cost":0.005,"tokens":{"input":500,"output":50,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_09', 'ses_main', 'assistant', 9, 1793178331000, 1793178331000, '{"time":{"created":1793178331000,"streamed":1793178340000,"completed":1793178340000},"agent":"build","model":{"id":"acme/coder-2","providerID":"acme","variant":"default"},"content":[{"type":"tool","id":"call-shell-3","name":"shell","executed":false,"state":{"status":"pending","input":{"command":"sleep 60"}},"time":{"created":1793178332000}}],"finish":"stop","cost":0.003,"tokens":{"input":300,"output":30,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_main_10', 'ses_main', 'synthetic', 10, 1793178341000, 1793178341000, '{"time":{"created":1793178341000},"text":"<system-reminder>Reminder at the end.</system-reminder>"}');
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_child1', 'prj_alpha', 'ses_main', 'ses_child1', '/home/user/alpha', 'Explore the repo', '1.0.0', 'explore', '{"id":"acme/coder-1","providerID":"acme"}', 1793178303000, 1793178309000, 1793178309000);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_child1_01', 'ses_child1', 'user', 1, 1793178303000, 1793178303000, '{"time":{"created":1793178303000},"text":"Look around the repository","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_child1_02', 'ses_child1', 'assistant', 2, 1793178304000, 1793178304000, '{"time":{"created":1793178304000,"streamed":1793178309000,"completed":1793178309000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"text","text":"Two crates: a lib and a binary."}],"finish":"stop","cost":0.001,"tokens":{"input":100,"output":20,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_child2', 'prj_alpha', 'ses_main', 'ses_child2', '/home/user/alpha', 'Review the code', '1.0.0', 'review', '{"id":"acme/coder-1","providerID":"acme"}', 1793178312000, 1793178328000, 1793178328000);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_child2_01', 'ses_child2', 'user', 1, 1793178312000, 1793178312000, '{"time":{"created":1793178312000},"text":"Review the change","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_child2_02', 'ses_child2', 'assistant', 2, 1793178313000, 1793178313000, '{"time":{"created":1793178313000,"streamed":1793178328000,"completed":1793178328000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"text","text":"Looks good to me."}],"finish":"stop","cost":0.001,"tokens":{"input":100,"output":20,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_untitled', 'prj_alpha', NULL, 'ses_untitled', '/home/user/alpha', NULL, '1.0.0', NULL, '{"id":"acme/coder-1","providerID":"acme"}', 1792504800000, 1792505400000, 1792505400000);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_untitled_01', 'ses_untitled', 'user', 1, 1792504800000, 1792504800000, '{"time":{"created":1792504800000},"text":"\nRename the config key\nand update the docs","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_untitled_02', 'ses_untitled', 'assistant', 2, 1792504802000, 1792504802000, '{"time":{"created":1792504802000,"streamed":1792504809000,"completed":1792504809000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"text","text":"Renamed."}],"finish":"stop","cost":0.0005,"tokens":{"input":50,"output":5,"reasoning":0,"cache":{"read":0,"write":0}}}');
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_long', 'prj_beta', NULL, 'ses_long', '/home/user/work/beta-tool', NULL, '1.0.0', NULL, '{"id":"acme/coder-1","providerID":"acme"}', 1792929600000, 1792929900000, NULL);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_long_01', 'ses_long', 'user', 1, 1792929600000, 1792929600000, '{"time":{"created":1792929600000},"text":"Investigate why the continuous integration pipeline keeps timing out on the nightly build of the beta tool","files":[],"agents":[]}');
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_blank', 'prj_beta', NULL, 'ses_blank', '/home/user/work/beta-tool', NULL, '1.0.0', NULL, '{"id":"acme/coder-1","providerID":"acme"}', 1792656000000, 1792656000000, 1792656000000);
INSERT INTO session_v2 (id, project_id, parent_id, slug, directory, title, version, agent, model, time_created, time_updated, time_idle) VALUES ('ses_bad', 'prj_beta', NULL, 'ses_bad', '/home/user/work/beta-tool', 'Broken rows', '1.0.0', NULL, '{"id":"acme/coder-1","providerID":"acme"}', 1793008800000, 1793009400000, 1793009400000);
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_bad_01', 'ses_bad', 'user', 1, 1793008800000, 1793008800000, '{"time":{"created":1793008800000},"text":"Hello","files":[],"agents":[]}');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_bad_02', 'ses_bad', 'assistant', 2, 1793008801000, 1793008801000, '{not json');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_bad_03', 'ses_bad', 'user', 3, 1793008802000, 1793008802000, '[]');
INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('msg_bad_04', 'ses_bad', 'assistant', 4, 1793008803000, 1793008803000, '{"time":{"created":1793008803000,"streamed":1793008804000,"completed":1793008804000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"default"},"content":[{"type":"text","text":"Still here."}],"finish":"stop","cost":0.0001,"tokens":{"input":10,"output":2,"reasoning":0,"cache":{"read":0,"write":0}}}');
