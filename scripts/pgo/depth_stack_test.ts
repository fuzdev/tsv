/**
 * The depth gate's stack facts (`scripts/pgo/depth_stack.ts`): the PE stack-field reader and
 * the reserve writer over synthetic headers of both optional-header widths — the writer
 * through a round trip with the reader, both refusing bytes that are not a PE image, the
 * writer refusing an image whose commit exceeds the new reserve — the two copies' reserves
 * and the threads Windows' rule gives each pass under them, and which spawn errors read as a
 * copy that did not start. A field read or written at the wrong offset or width, or a
 * reserve that leaves a pass off its request, fails here, not as a Windows row graded on
 * the wrong thread.
 */

import { deepStrictEqual, ok, strictEqual, throws } from 'node:assert';

import {
	CONTRACT_STACK_MB,
	CONTROL_STACK_MB,
	CONTROL_STACK_RESERVE,
	GATE_STACK_MB,
	GATE_STACK_RESERVE,
	MIB,
	pe_set_stack_reserve,
	pe_stack_sizes,
	spawn_did_not_start,
	stack_request_bytes,
	windows_thread_bytes
} from './depth_stack.ts';

/** The PE signature's offset in the synthetic images (`e_lfanew`), off the minimum on
 * purpose, so a reader that assumed it would misread. */
const PE_OFFSET = 0x80;

const OPTIONAL = PE_OFFSET + 4 + 20;

/** A synthetic image: an `MZ` DOS header naming the `PE\0\0` signature, a COFF header
 * declaring the optional header's standard size, and an optional header of `magic` with
 * every byte but the magic set to a filler, then `reserve` at offset 72 and `commit` after
 * it, at the magic's width. */
function synthetic_pe(magic: 0x10b | 0x20b, reserve: number, commit = 4096): Uint8Array {
	const size = magic === 0x20b ? 240 : 224;
	const bytes = new Uint8Array(OPTIONAL + size).fill(0xa5);
	const view = new DataView(bytes.buffer);
	view.setUint16(0, 0x5a4d, true);
	view.setUint32(0x3c, PE_OFFSET, true);
	view.setUint32(PE_OFFSET, 0x00004550, true);
	view.setUint16(PE_OFFSET + 4 + 16, size, true);
	view.setUint16(OPTIONAL, magic, true);
	if (magic === 0x20b) {
		view.setBigUint64(OPTIONAL + 72, BigInt(reserve), true);
		view.setBigUint64(OPTIONAL + 80, BigInt(commit), true);
	} else {
		view.setUint32(OPTIONAL + 72, reserve, true);
		view.setUint32(OPTIONAL + 76, commit, true);
	}
	return bytes;
}

Deno.test('pe_stack_sizes reads both optional-header widths', () => {
	deepStrictEqual(pe_stack_sizes(synthetic_pe(0x20b, 8 * MIB, 8192)), {
		reserve: 8 * MIB,
		commit: 8192
	});
	deepStrictEqual(pe_stack_sizes(synthetic_pe(0x10b, MIB, 4096)), { reserve: MIB, commit: 4096 });
	// a reserve that needs the upper half of PE32+'s u64
	strictEqual(pe_stack_sizes(synthetic_pe(0x20b, 2 ** 33))?.reserve, 2 ** 33);
});

Deno.test('pe_stack_sizes reads through a subarray at an offset', () => {
	const image = synthetic_pe(0x20b, 3 * MIB);
	const padded = new Uint8Array(image.length + 16);
	padded.set(image, 16);
	strictEqual(pe_stack_sizes(padded.subarray(16))?.reserve, 3 * MIB);
});

Deno.test('pe_stack_sizes refuses what is not a PE image', () => {
	strictEqual(pe_stack_sizes(new Uint8Array(0)), null);
	strictEqual(pe_stack_sizes(new Uint8Array(0x40)), null); // no `MZ`
	const no_signature = synthetic_pe(0x20b, 1);
	no_signature[PE_OFFSET] = 0;
	strictEqual(pe_stack_sizes(no_signature), null);
	const bad_magic = synthetic_pe(0x20b, 1);
	bad_magic[OPTIONAL] = 0x07;
	strictEqual(pe_stack_sizes(bad_magic), null);
	// the headers cut short of the commit field
	strictEqual(pe_stack_sizes(synthetic_pe(0x20b, 1).subarray(0, OPTIONAL + 87)), null);
	strictEqual(pe_stack_sizes(synthetic_pe(0x10b, 1).subarray(0, OPTIONAL + 79)), null);
});

Deno.test(
	'pe_stack_sizes refuses a COFF header whose optional header ends before the fields',
	() => {
		for (const [magic, fields_end] of [
			[0x20b, 88],
			[0x10b, 80]
		] as const) {
			const image = synthetic_pe(magic, MIB);
			const view = new DataView(image.buffer);
			view.setUint16(PE_OFFSET + 4 + 16, fields_end - 1, true);
			strictEqual(pe_stack_sizes(image), null);
			strictEqual(pe_set_stack_reserve(image, MIB).ok, false);
			view.setUint16(PE_OFFSET + 4 + 16, fields_end, true);
			strictEqual(pe_stack_sizes(image)?.reserve, MIB);
		}
	}
);

for (const magic of [0x20b, 0x10b] as const) {
	const width = magic === 0x20b ? 'PE32+' : 'PE32';
	Deno.test(`pe_set_stack_reserve round-trips through the reader (${width})`, () => {
		for (const reserve of [CONTROL_STACK_RESERVE, GATE_STACK_RESERVE]) {
			const image = synthetic_pe(magic, MIB);
			const before = image.slice();
			ok(pe_set_stack_reserve(image, reserve).ok);
			deepStrictEqual(pe_stack_sizes(image), { reserve, commit: 4096 });
			// only the reserve field's own bytes moved
			const field = OPTIONAL + 72;
			const field_end = field + (magic === 0x20b ? 8 : 4);
			for (let i = 0; i < image.length; i++) {
				if (i < field || i >= field_end) strictEqual(image[i], before[i], `byte ${i} moved`);
			}
		}
	});
}

Deno.test('pe_set_stack_reserve clears the upper half of a PE32+ field', () => {
	const image = synthetic_pe(0x20b, 2 ** 40);
	ok(pe_set_stack_reserve(image, CONTROL_STACK_RESERVE).ok);
	strictEqual(pe_stack_sizes(image)?.reserve, CONTROL_STACK_RESERVE);
});

Deno.test('pe_set_stack_reserve refuses what it cannot write', () => {
	const not_pe = new Uint8Array(0x200);
	strictEqual(pe_set_stack_reserve(not_pe, CONTROL_STACK_RESERVE).ok, false);
	ok(
		not_pe.every((b) => b === 0),
		'a refused write wrote nothing'
	);
	throws(() => pe_set_stack_reserve(synthetic_pe(0x10b, 1), 2 ** 32), RangeError);
	throws(() => pe_set_stack_reserve(synthetic_pe(0x20b, 1), 0), RangeError);
	throws(() => pe_set_stack_reserve(synthetic_pe(0x20b, 1), 1.5), RangeError);
});

Deno.test('pe_set_stack_reserve refuses a reserve below the image commit', () => {
	for (const magic of [0x20b, 0x10b] as const) {
		const image = synthetic_pe(magic, 8 * MIB, CONTROL_STACK_RESERVE + 4096);
		const before = image.slice();
		const set = pe_set_stack_reserve(image, CONTROL_STACK_RESERVE);
		strictEqual(set.ok, false);
		ok(!set.ok && set.reason.includes('commit'), 'the refusal names the commit');
		deepStrictEqual(image, before, 'a refused write wrote nothing');
		// a commit equal to the reserve is a thread of exactly the reserve, so it is allowed
		ok(pe_set_stack_reserve(image, CONTROL_STACK_RESERVE + 4096).ok);
	}
});

Deno.test("each copy's reserve sits just above its pass's request", () => {
	strictEqual(CONTROL_STACK_RESERVE, 576 * 1024);
	strictEqual(GATE_STACK_RESERVE, 1_900_544);
	for (const [stack_mb, reserve] of [
		[CONTROL_STACK_MB, CONTROL_STACK_RESERVE],
		[GATE_STACK_MB, GATE_STACK_RESERVE]
	] as const) {
		const request = stack_request_bytes(stack_mb);
		strictEqual(reserve % (64 * 1024), 0, 'a multiple of the allocation granularity');
		ok(reserve > request && reserve - request <= 64 * 1024, `${reserve} just above ${request}`);
		// under its own copy, the pass's thread is that reserve: the commit falls under it
		strictEqual(windows_thread_bytes(request, reserve), reserve);
	}
	// the gate keeps its margin: its thread is under the contract
	ok(GATE_STACK_RESERVE < CONTRACT_STACK_MB * MIB);
});

Deno.test('windows_thread_bytes rounds a commit at or above the reserve up to a MiB', () => {
	const gate = stack_request_bytes(GATE_STACK_MB);
	// the gate under the control copy, or under one reserve for both passes
	strictEqual(windows_thread_bytes(gate, CONTROL_STACK_RESERVE), 2 * MIB);
	// a commit page-rounded onto the reserve exactly
	strictEqual(windows_thread_bytes(CONTROL_STACK_RESERVE - 100, CONTROL_STACK_RESERVE), MIB);
	// a stock node.exe's larger reserve sizes every smaller request
	strictEqual(windows_thread_bytes(gate, 8 * MIB), 8 * MIB);
});

Deno.test('spawn_did_not_start reads only a start failure as one', () => {
	const error = (code?: string): Error & { code?: string } =>
		Object.assign(new Error('spawn'), { code });
	for (const code of ['ENOENT', 'EACCES', 'EPERM']) ok(spawn_did_not_start(error(code)), code);
	for (const code of ['ENOBUFS', 'ETIMEDOUT', undefined]) {
		ok(!spawn_did_not_start(error(code)), String(code));
	}
});
