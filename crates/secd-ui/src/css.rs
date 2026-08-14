//! Token-only layout glue. No chrome, no palette.

pub const APP_CSS: &str = "\
.secd-auth-form{display:flex;flex-direction:column;gap:16px;width:100%}\
.secd-btn-block{width:100%;justify-content:center}\
.secd-stack{display:flex;flex-direction:column;gap:16px}\
.secd-row{display:flex;align-items:center;gap:8px;flex-wrap:wrap}\
.secd-grid-2{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:16px;align-items:start}\
.secd-list{display:flex;flex-direction:column;gap:8px}\
.secd-field-actions{display:flex;align-items:center;gap:8px;margin-top:8px}\
.secd-name{justify-content:flex-start}\
.secd-remembered{font-size:13px;color:var(--color-text-muted);\
border:1px solid var(--color-border-soft);border-radius:var(--radius-md);\
padding:8px 12px}\
.secd-overlay{position:fixed;inset:0;z-index:60;display:flex;\
align-items:flex-start;justify-content:center;padding:10vh 16px 16px;\
background:rgba(0,0,0,.55)}\
.secd-modal{width:100%;max-width:440px}\
@media (max-width:900px){.secd-grid-2{grid-template-columns:minmax(0,1fr)}}\
";
