/**
 * The thread-stack facts the PGO depth gate shares between its orchestrator (`pgo.ts depth`)
 * and its runner (`addon_depth.ts`): the threads each pass asks for, the "cannot grade here"
 * exit and the stderr line that must come with it, the thread Windows gives a worker, and
 * the PE image's `SizeOfStackReserve` — read to learn that thread, and written into the
 * scratch copies of node.exe that make a worker's `resourceLimits.stackSizeMb` size its
 * thread there too.
 *
 * Pure (no I/O), so it unit-tests over synthetic headers (`depth_stack_test.ts`).
 *
 * @module
 */

export const MIB = 1024 * 1024;

/** The contract's thread in an optimized build, MiB. */
export const CONTRACT_STACK_MB = 2;

/** The gate pass's thread: the contract's less the tenth the native gate keeps, MiB. */
export const GATE_STACK_MB = CONTRACT_STACK_MB * 0.9;

/** The control pass's thread, MiB: far below what the deepest shapes need, so it must
 * overflow — yet above the floor Node starts a worker on (its own stack buffer, 192 KiB). */
export const CONTROL_STACK_MB = 0.5;

/** `addon_depth.ts`'s exit when it cannot grade the contract on this machine, which
 * `pgo.ts depth` reads, on Windows alone, as "fall back to the plain-profile native gate". */
export const CANNOT_GRADE_EXIT = 3;

/** The stderr line `addon_depth.ts` prints with `CANNOT_GRADE_EXIT`; `pgo.ts depth` falls
 * back only on the exit AND this line, so an unrelated process death that happens to exit 3
 * (the MSVC CRT's `abort()` does) still fails. */
export const CANNOT_GRADE_LINE = 'addon_depth: cannot grade the stack contract here';

/** Windows' page (x64 and arm64), which a thread's commit is rounded up to. */
const PAGE = 4096;

/** Windows' allocation granularity, which a reserve is a multiple of. */
const ALLOCATION_GRANULARITY = 64 * 1024;

/** The stack size, bytes, a worker asks for with `resourceLimits.stackSizeMb: stack_mb`. */
export const stack_request_bytes = (stack_mb: number): number => Math.round(stack_mb * MIB);

const round_up = (bytes: number, unit: number): number => Math.ceil(bytes / unit) * unit;

/**
 * The thread, bytes, Windows gives a worker asking for `request` bytes in a process whose
 * image reserves `reserve`: libuv passes the size as the thread's COMMIT (page-rounded),
 * and a commit under the reserve leaves the thread the reserve, while a commit at or above
 * it makes the thread's reserve the commit rounded up to a MiB.
 */
export function windows_thread_bytes(request: number, reserve: number): number {
	const commit = round_up(request, PAGE);
	return commit >= reserve ? round_up(commit, MIB) : reserve;
}

/** The smallest reserve a worker asking for `stack_mb` runs on as itself: the first
 * multiple of the allocation granularity strictly above the page-rounded commit, so the
 * commit falls under it and the thread is that reserve, at most 64 KiB above the page-rounded commit. */
export const reserve_above = (stack_mb: number): number =>
	(Math.floor(round_up(stack_request_bytes(stack_mb), PAGE) / ALLOCATION_GRANULARITY) + 1) *
	ALLOCATION_GRANULARITY;

/** The control copy's reserve (576 KiB): just above the control's request, so its thread
 * stays near it. */
export const CONTROL_STACK_RESERVE = reserve_above(CONTROL_STACK_MB);

/** The gate copy's reserve (1,900,544 B, 1.8125 MiB): just above the gate's request, so its
 * thread keeps nearly all of the contract's tenth margin. */
export const GATE_STACK_RESERVE = reserve_above(GATE_STACK_MB);

/** The spawn error codes that mean an executable could not be started at all — the only
 * spawn errors `pgo.ts depth` reads, on Windows, as "the node.exe copy does not run here"
 * rather than as the gate's own failure (ENOBUFS, say, from a flood of failure output). */
const NOT_STARTED_CODES: ReadonlySet<string> = new Set(['ENOENT', 'EACCES', 'EPERM']);

/** Whether a spawn's `error` means the executable did not start. */
export const spawn_did_not_start = (error: Error & { code?: string }): boolean =>
	NOT_STARTED_CODES.has(error.code ?? '');

/** A PE image's stack fields: where `SizeOfStackReserve` and `SizeOfStackCommit` live and
 * how wide they are. */
interface StackFields {
	reserve_offset: number;
	commit_offset: number;
	wide: boolean;
}

/** Where a PE image's stack fields live, or `null` when the bytes are not a PE image's
 * headers. The DOS header's `e_lfanew` names the `PE\0\0` signature; the optional header
 * follows the 20-byte COFF header (whose `SizeOfOptionalHeader` must cover both fields) and
 * holds the reserve at offset 72 in both PE32 (`u32`, the commit at 76) and PE32+ (`u64`,
 * the commit at 80). */
function stack_fields(bytes: Uint8Array): StackFields | null {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (bytes.length < 0x40 || view.getUint16(0, true) !== 0x5a4d) return null; // `MZ`
	const pe = view.getUint32(0x3c, true);
	const optional = pe + 4 + 20;
	if (optional + 2 > bytes.length || view.getUint32(pe, true) !== 0x00004550) return null;
	const magic = view.getUint16(optional, true);
	const wide = magic === 0x20b;
	if (!wide && magic !== 0x10b) return null;
	const fields_end = wide ? 88 : 80;
	const declared = view.getUint16(pe + 4 + 16, true);
	if (declared < fields_end || optional + fields_end > bytes.length) return null;
	return { reserve_offset: optional + 72, commit_offset: optional + (wide ? 80 : 76), wide };
}

const read_field = (view: DataView, offset: number, wide: boolean): number =>
	wide ? Number(view.getBigUint64(offset, true)) : view.getUint32(offset, true);

/** A PE image's `SizeOfStackReserve` and `SizeOfStackCommit`, from its leading bytes, or
 * `null` when they are not a PE image's headers. */
export function pe_stack_sizes(bytes: Uint8Array): { reserve: number; commit: number } | null {
	const fields = stack_fields(bytes);
	if (!fields) return null;
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	return {
		reserve: read_field(view, fields.reserve_offset, fields.wide),
		commit: read_field(view, fields.commit_offset, fields.wide)
	};
}

/**
 * Set a PE image's `SizeOfStackReserve` in place, at the width its optional header
 * declares. `SizeOfStackCommit` is left as it is and must not exceed the new reserve (the
 * loader would otherwise grow the main thread's reserve past it, a thread no rule here
 * models), so such an image is refused. The header checksum is left as it was: Windows
 * verifies it for drivers and boot images, never for an executable it starts.
 *
 * @param bytes - the image, or at least its headers
 * @param reserve - the new reserve, bytes; must fit the field (PE32: below 4 GiB)
 * @returns `ok: false` with the reason when nothing was written: the bytes are not a PE
 *   image's headers, or the image's commit exceeds `reserve`
 * @mutates bytes - the reserve field
 * @throws RangeError when `reserve` does not fit the field
 */
export function pe_set_stack_reserve(
	bytes: Uint8Array,
	reserve: number
): { ok: true } | { ok: false; reason: string } {
	const fields = stack_fields(bytes);
	if (!fields) return { ok: false, reason: 'not a PE image' };
	if (!Number.isSafeInteger(reserve) || reserve <= 0 || (!fields.wide && reserve > 0xffff_ffff)) {
		throw new RangeError(`a stack reserve of ${reserve} does not fit the PE field`);
	}
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const commit = read_field(view, fields.commit_offset, fields.wide);
	if (commit > reserve) {
		return { ok: false, reason: `its stack commit (${commit} B) exceeds a ${reserve} B reserve` };
	}
	if (fields.wide) view.setBigUint64(fields.reserve_offset, BigInt(reserve), true);
	else view.setUint32(fields.reserve_offset, reserve, true);
	return { ok: true };
}
