<script lang="ts">
type T = {
[k: string] /* x */: // c
number;
};

interface I {
[k: string] /* d */: // e
string;
}

// `]`→`:` itself a line comment: two stacked continuations — the value `:` drops
// one level, then its type drops another, so the type lands two levels in.
interface J {
[k: string] // f
: // g
string;
}
</script>
