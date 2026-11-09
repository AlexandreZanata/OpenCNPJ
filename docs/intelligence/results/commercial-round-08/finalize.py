#!/usr/bin/env python3
"""Reconcile completed immutable CLI outputs; never execute or relabel a query."""
from collections import Counter, defaultdict
from datetime import datetime, timezone
from itertools import zip_longest
from pathlib import Path
import hashlib
import json
import re
import subprocess
import unicodedata

ROOT = Path(__file__).resolve().parents[4]
BASE = Path(__file__).resolve().parent
SET = ROOT / 'crates/opencnpj-lab/eval/commercial-round-08/questions.jsonl'

def read(path):
    return json.loads(path.read_text())

def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def fold(text):
    return ' '.join(''.join(c for c in unicodedata.normalize('NFKD', text.lower()) if not unicodedata.combining(c)).split())

def save(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')

def write_report(result):
    baseline = result['national_baseline']
    final = result['final_frozen_label_agreement']
    reuse = result['backend_evidence_reuse']
    patch = result['separate_adjudication_patch']
    lines = [
        '# Round 08 completed and semantic defects reviewed', '',
        'Review date: 2026-11-08. Data anchor: 2026-10-11; competence 2026-10.',
        'Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable',
        'completion: [index.json](index.json). No commit or push was made.', '',
        '## Outcome', '',
        f'- The original national run completed all **50,000/50,000** questions: **{baseline["interpretation_correct"]:,}/50,000** original label matches.',
        f'- Every text was reinterpreted with the corrected final binary: **{final["interpretation_correct"]:,}/50,000** matches against the unchanged labels.',
        '- **1,600 original closure labels are semantically defective.** They treat a start-of-activity period as the closure period and omit the start-date filter. The corrected parser clarifies instead of answering that different question.',
        '- The other **136 disagreements** are municipality-typo questions that clarify. No final mismatch emits an executable request.',
        f'- The separate, hash-bound semantic adjudication patch yields **{patch["agreement_with_documented_replacement_labels"]:,}/50,000** agreement. This is a secondary development-set score, not blind or human validation.',
        '- **20/20 focused review cases** matched; all **13 executable review requests** succeeded on the actual local artifacts.',
        '- Final delivery gate: **300 passed, 0 failed, 5 ignored**; formatting and Clippy passed.', '',
        'The lower raw frozen-label score is explained by retaining the 1,600 bad',
        'labels for audit. They were not silently changed to fit predictions.',
        '`label-adjudications.jsonl` records replacement decisions, original-gold',
        'hashes, authorship and semantic reasons separately. Human validation stays',
        'deferred to product refinement, as requested. H-blind was not opened or run.', '',
        '## Corrections made', '',
        '1. Date literals retain separators while entity names are masked. Dates no longer become eight-digit CNPJ roots; invalid calendar dates clarify.',
        '2. Historical stock needs an explicit monthly competence. A year does not silently choose December; years with no available snapshots are rejected.',
        '3. Closure periods and start-of-activity filters are resolved separately. A missing closure period clarifies; explicit periods retain both constraints.',
        '4. Start-date filters survive on lists and stock counts.',
        '5. Informal size words such as grandonas clarify instead of being discarded.',
        '6. Numeric capital/population filters do not become identifier lookups. Explicit and standalone CNPJ roots remain supported.', '',
        'Focused regression tests exercise these behaviors. Previously implemented',
        'entity-name fixes remain in place, including Contagem and Não-Me-Toque.',
        'The new review dialogue covers both cities without editing the old set.', '',
        '## Independent frozen suites', '',
        '- Adversarial: **198 -> 210 / 304**.',
        '- Informal: **116 -> 118 / 202**.',
        '- Dialogues: **81 -> 81 / 122**.',
        '- Out of domain: **377 -> 377 / 400**.', '',
        'All 14 previously emitted executable requests against clarify/reject',
        'labels now clarify. The legacy counter is called out_of_policy_execution,',
        'but in interpret-only runs it counts executable requests, not actual',
        'backend calls. Improvements are real but limited; the unchanged dialogue',
        'and out-of-domain scores still show substantial language gaps.', '',
        'Prior commercial rounds were also replayed with the final binary. Their',
        'raw agreement falls where the same closure-label defect exists; see the',
        'per-round counts and label-defect audit in index.json. Round 01 remains',
        '1000/1000. A repeated high template score is not universal Portuguese coverage.', '',
        '## Backend evidence and completion integrity', '',
        f'- The baseline executed **{baseline["gold_backend_ok"]:,} typed reference requests** successfully. Well-typed reference IR does not prove its text label is semantically correct.',
        f'- The finalizer checked **{reuse["identical_typed_requests_with_successful_baseline_execution"]:,} accepted final requests** against exactly equal typed baseline requests with successful executions.',
        '- Existing executions were reused only for equal IR on the same pinned data. No second full national backend run with the corrected parser is claimed.',
        '- The 20 new review cases were executed separately, rather than borrowing results from different filters.',
        '- Every original input has exactly one baseline result and one final interpretation, in order, with matching ID and text. No input record was dropped.',
        '- Original input/export/catalogue hashes match their manifest; final source and evidence hashes are retained.', '',
        'Baseline differential evidence:', '',
    ]
    for key in ('verification_engine_match', 'verification_marginal_match', 'verification_no_independent_reference', 'verification_not_numeric', 'verification_single_backend'):
        lines.append(f'- {key}: {baseline.get(key, 0):,}.')
    lines += [
        '', 'Single-backend and missing-reference results are not independent',
        'differential proofs. No throughput, latency, memory or target-VPS claim is made.', '',
        '## Dataset and remaining adjustments', '',
        '- The 50,000 texts are distinct within this set. There are 363 earlier-text overlaps: 359 dialogue turns and four fixed small-talk questions. Repeated follow-ups may have different contexts.',
        '- 200 session names are reused from round 07. Dataset evaluations reset state independently; this is a documentation/provenance defect, not shared conversation input.',
        '- The card now correctly states 45,911 items without filler repair, not 15,911. Filler repairs do not establish semantic independence.',
        '- Next label revision should apply and review the explicit adjudication patch while retaining the frozen original as evidence.',
        '- Prioritize the remaining adversarial/informal/dialogue failure families, semantically varied questions and supported clarification options. Do not force execution of ambiguous place typos to improve a score.',
        '- Durable evaluator checkpoints/resume, confidence calibration and application/API/context integration remain product work. Human refinement and validation remain later stages.', '',
        'This round is complete. Product readiness and coverage of every possible',
        'commercial phrasing are not established by these synthetic evaluations.', '',
        '## Execution handling', '',
        'The original national process was preserved through completion. Automatic',
        'approval review rejected four parallel backend shards with process suspension,',
        'and later rejected another concurrent evaluation batch, citing resource and',
        'interruption risks. Neither rejected action ran. Final re-evaluations ran',
        'sequentially only after the original national executable exited.', '',
        '## Short handoff prompt', '',
        '> Read AGENTS.md and commercial-round-08/{REPORT,COMMANDS,index}. Preserve',
        '> frozen inputs; use the explicit label-adjudication patch as a separate',
        '> version. Improve remaining independent-suite failures with focused',
        '> semantic cases, retain ambiguous-place clarifications, rerun the gate',
        '> and record before/after evidence. Avoid another large template round.',
        '> Keep H-blind intact, defer human validation, and do not commit or push.', '',
    ]
    (BASE / 'REPORT.md').write_text('\n'.join(lines))

def main():
    assert (BASE / 'national/summary.json').exists(), 'The original national run is not complete yet'
    baseline = read(BASE / 'national/summary.json')
    final = read(BASE / 'post-review-interpretation/summary.json')
    audit = read(BASE / 'dataset-audit.json')
    checks = read(BASE / 'checks.json')
    runs = read(BASE / 'evaluation-runs.json')
    review = read(BASE / 'review-regressions/summary.json')
    assert set(checks) == {'fmt', 'clippy', 'test', 'build'}
    assert all(c['exit_code'] == 0 for c in checks.values())
    assert len(runs) == 13 and all(r['exit_code'] == 0 for r in runs.values())
    binary_sha256 = sha(Path('/data/opencnpj-target/release/opencnpj-intel'))
    assert all(r['binary_sha256'] == binary_sha256 for r in runs.values()), 'Stale evaluation executable'
    assert runs['replay-round-01']['counts']['interpretation_correct'] == 1000
    assert review['counts']['items'] == review['counts']['interpretation_correct'] == 20
    assert review['counts']['correct_executed_answer'] == 13
    assert review['counts'].get('gold_backend_error', 0) == 0
    assert baseline['counts']['items'] == final['counts']['items'] == audit['items'] == 50_000
    assert baseline['set_sha256'] == final['set_sha256'] == sha(SET)
    assert baseline['data_anchor'] == final['data_anchor'] == '2026-10-11'
    manifest = read(SET.parent / 'manifest.json')
    for path, digest in manifest['sha256'].items():
        assert sha(ROOT / path) == digest, f'Original input changed: {path}'
    assert baseline['counts'].get('invalid_gold', 0) == 0
    assert baseline['counts'].get('gold_backend_error', 0) == 0
    assert baseline['counts'].get('gold_backend_mismatch', 0) == 0
    assert baseline['counts'].get('gold_backend_reference_error', 0) == 0
    assert baseline['counts']['gold_backend_ok'] == 44_750

    counts = Counter()
    categories = defaultdict(Counter)
    ids = set()
    defects = set(audit['label_defect_ids'])
    adjudications = {row['id']: row for row in map(json.loads, (BASE / 'label-adjudications.jsonl').open())}
    assert set(adjudications) == defects
    partition = Counter()
    changes = Counter()
    reuse = Counter()
    with SET.open() as source, (BASE / 'national/results.jsonl').open() as old, (BASE / 'post-review-interpretation/results.jsonl').open() as new:
        for lines in zip_longest(source, old, new):
            assert all(line is not None and line.endswith('\n') for line in lines), 'Incomplete or missing result row'
            item, previous, current = map(json.loads, lines)
            assert item['id'] == previous['id'] == current['id']
            assert item['text'] == previous['text'] == current['text']
            assert item['id'] not in ids, 'Duplicate result ID'
            ids.add(item['id'])
            category = categories[current['category']]
            counts['items'] += 1
            category['items'] += 1
            prediction = current['prediction']
            request = prediction['request']
            correct = current['interpretation_correct']
            expected = item['gold']['decision']
            assert expected == previous['expected_decision'] == current['expected_decision']
            if current['prediction']['request'] != previous['prediction']['request'] or current['prediction']['decision'] != previous['prediction']['decision']:
                changes['changed_interpretations'] += 1
            if (request is not None if expected in ('execute', 'execute_with_note') else prediction['decision'] == expected):
                counts['decision_correct'] += 1
            if item['gold'].get('act') is not None:
                counts['act_items'] += 1
                if prediction['act'] == item['gold']['act']:
                    counts['act_correct'] += 1
            if request is not None and expected not in ('execute', 'execute_with_note'):
                counts['out_of_policy_execution'] += 1
            if correct:
                counts['interpretation_correct'] += 1
                category['interpretation_correct'] += 1
            else:
                counts['interpretation_wrong'] += 1
                if item['id'] in defects:
                    assert prediction['decision'] == 'clarify' and request is None
                    partition['confirmed_closure_label_defects'] += 1
                elif 'place_typo' in item['meta']['tags'] and prediction['decision'] == 'clarify' and request is None:
                    partition['place_typos_requiring_clarification'] += 1
                else:
                    raise AssertionError(f'Unexplained final mismatch: {item["id"]}')
            if item['id'] in defects:
                assert not correct, 'A frozen defective label was silently matched'
                patch = adjudications[item['id']]
                original = json.dumps(item['gold'], sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
                assert hashlib.sha256(original).hexdigest() == patch['original_gold_sha256']
                assert patch['frozen_set_sha256'] == baseline['set_sha256']
                assert prediction['decision'] == patch['replacement_gold']['decision']
            if request is not None:
                candidates = [(previous['prediction']['request'], previous['predicted_execution']), (previous['gold_canonical'], previous['gold_execution'])]
                matches = [execution for typed, execution in candidates if typed == request and execution and execution['status'] == 'ok']
                assert matches, f'New backend execution is required: {item["id"]}'
                assert correct, f'Wrong accepted request requires investigation: {item["id"]}'
                reuse['identical_typed_requests_with_successful_baseline_execution'] += 1
                reuse['verification_' + matches[0].get('verification', 'unknown')] += 1
    assert len(ids) == 50_000
    assert dict(counts) == final['counts'], (dict(counts), final['counts'])
    assert partition['confirmed_closure_label_defects'] == len(defects) == 1600
    assert sum(partition.values()) == counts['interpretation_wrong']
    test_rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', (BASE / 'test.log').read_text())
    totals = dict(zip(('passed', 'failed', 'ignored'), (sum(int(row[i]) for row in test_rows) for i in range(3))))
    assert totals == {'passed': 300, 'failed': 0, 'ignored': 5}
    prior_audit = {}
    for number in range(1, 9):
        path = ROOT / f'crates/opencnpj-lab/eval/commercial-round-{number:02}/questions.jsonl'
        with path.open() as stream:
            rows = (json.loads(line) for line in stream)
            bad = sum(row['meta']['category'] == 'closures' and 'com inicio de atividade ' in fold(row['text']) and not row.get('gold', {}).get('ir', {}).get('scope', {}).get('opened') for row in rows)
        prior_audit[f'commercial-round-{number:02}'] = {'closure_label_defects': bad, 'set_sha256': sha(path)}
    result = {
        'round': 'commercial-round-08', 'status': 'complete',
        'review_date': '2026-11-08', 'finalized_at_utc': datetime.now(timezone.utc).isoformat(),
        'source_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'uncommitted_changes': True, 'human_validation': 'deferred to product refinement',
        'blind_set_used': False, 'production_readiness': 'not established',
        'set_sha256': sha(SET), 'snapshot_sha256': baseline['snapshot_sha256'],
        'final_binary_sha256': binary_sha256,
        'national_baseline': baseline['counts'], 'final_frozen_label_agreement': dict(counts),
        'mismatch_partition': dict(partition), 'interpretation_changes_from_baseline': dict(changes),
        'separate_adjudication_patch': {
            'items': len(adjudications), 'original_questions_modified': False,
            'agreement_with_documented_replacement_labels': counts['interpretation_correct'] + len(adjudications),
            'note': 'Secondary development-set agreement after explicit semantic adjudication; not blind or human validation.',
        },
        'backend_evidence_reuse': dict(reuse),
        'backend_evidence_note': 'Final texts were reinterpreted independently. Existing baseline backend results were reused only for exactly equal canonical typed requests on the same pinned artifacts; no second national backend run is claimed.',
        'review_regressions': review['counts'], 'delivery_gate': {**totals, **{name: c['exit_code'] for name, c in checks.items()}},
        'frozen_suites': {name: runs[name]['counts'] for name in ('frozen-adv', 'frozen-informal', 'frozen-dialogues', 'frozen-ood')},
        'prior_round_replays': {name: value['counts'] for name, value in runs.items() if name.startswith('replay-round-')},
        'prior_round_label_audit': prior_audit,
        'counter_note': 'The legacy out_of_policy_execution counter measures emitted executable requests; interpret-only evaluations do not execute backends.',
    }
    save(BASE / 'index.json', result)
    write_report(result)
    source_hashes = {str(p.relative_to(ROOT)): sha(p) for p in sorted((ROOT / 'crates/opencnpj-nlu').rglob('*.rs'))}
    for p in sorted((ROOT / 'crates/opencnpj-nlu/lang').rglob('*.toml')):
        source_hashes[str(p.relative_to(ROOT))] = sha(p)
    for path in ('Cargo.toml', 'Cargo.lock', 'crates/opencnpj-nlu/Cargo.toml', 'crates/opencnpj-lab/src/questions.rs', 'crates/opencnpj-lab/src/query.rs'):
        source_hashes[path] = sha(ROOT / path)
    save(BASE / 'final-source-sha256.json', source_hashes)
    evidence = {str(p.relative_to(ROOT)): sha(p) for p in sorted(BASE.rglob('*')) if p.is_file() and p.name != 'evidence-sha256.json'}
    for path in (SET, SET.parent / 'questions.tsv', SET.parent / 'manifest.json', SET.parent / 'DATASET-CARD.md', SET.parent / 'review-regressions.jsonl', SET.parent / 'review-regressions-manifest.json'):
        evidence[str(path.relative_to(ROOT))] = sha(path)
    save(BASE / 'evidence-sha256.json', evidence)
    print(json.dumps({'status': result['status'], 'baseline': result['national_baseline'], 'final_interpretation': dict(counts), 'mismatches': dict(partition), 'backend_evidence_reuse': dict(reuse), 'review_regressions': review['counts'], 'gate': result['delivery_gate']}, ensure_ascii=False, indent=2))

if __name__ == '__main__':
    main()
