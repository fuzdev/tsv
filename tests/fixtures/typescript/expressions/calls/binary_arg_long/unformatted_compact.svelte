<script>
/* 100 chars total - call stays inline */
const a=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefghijklmnopqr);

/* 101 chars total - call wraps, binary stays inline */
const b=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefghijklmnopqrs);

/* 100 chars arg line - binary stays inline */
const c=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefg);

/* 101 chars arg line - binary breaks, continuation gets extra indent */
const d=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbabcdefgh);

/* Multi-operand chain - each continuation indented */
const e=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&&bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb&&cccccccccccccccccccccccccccccc);

/* || chain - same behavior */
const f=fn(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa||bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb||cccccccccccccccccccccccccccccc);
</script>
