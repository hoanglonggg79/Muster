import dnfile
from dncil.cil.body import CilMethodBody
from dncil.cil.body.reader import CilMethodBodyReaderBase
from dncil.cil.error import MethodBodyFormatError

P = "Sodium.dll"


class Reader(CilMethodBodyReaderBase):
    def __init__(self, pe, rva):
        self.pe = pe
        self.offset = self.pe.get_offset_from_rva(rva)

    def read(self, n):
        data = self.pe.get_data(self.pe.get_rva_from_offset(self.offset), n)
        self.offset += n
        return data

    def tell(self):
        return self.offset

    def seek(self, o):
        self.offset = o


pe = dnfile.dnPE(P)
md = pe.net.mdtables


def resolve(tok):
    if tok is None:
        return None
    table = tok >> 24
    rid = tok & 0xFFFFFF
    try:
        if table == 0x06:
            return "MethodDef: " + str(md.MethodDef.rows[rid - 1].Name)
        if table == 0x0A:
            r = md.MemberRef.rows[rid - 1]
            parent = r.Class.row if hasattr(r.Class, "row") else r.Class
            pname = getattr(parent, "Name", None) or getattr(parent, "TypeName", "?")
            return f"MemberRef: {pname}::{r.Name}"
        if table == 0x04:
            return "Field: " + str(md.Field.rows[rid - 1].Name)
        if table == 0x70:
            return "String: " + repr(str(md.UserStrings.rows[rid - 1].Value))
    except Exception as e:
        return f"tok({hex(tok)}) err={e}"
    return f"tok({hex(tok)})"


WANT = {"ArgonHashBinary", "ArgonHashString"}
for t in md.TypeDef:
    if str(t.TypeName) != "PasswordHash":
        continue
    for idx in t.MethodList:
        m = idx.row if hasattr(idx, "row") else idx
        name = str(m.Name)
        if name not in WANT or not m.Rva:
            continue
        print("=" * 78)
        print("METHOD:", name, "RVA", hex(m.Rva), "rid", hex(0x06000000 | (idx.row_index if hasattr(idx,'row_index') else 0)))
        try:
            body = CilMethodBody(Reader(pe, m.Rva))
        except MethodBodyFormatError as e:
            print("  body error:", e)
            continue
        for ins in body.instructions:
            op = ins.opcode.name
            extra = ""
            if isinstance(ins.operand, int) and op.startswith(("call", "newobj", "ldsfld", "ldfld", "ldstr")):
                extra = "   ->  " + str(resolve(ins.operand))
            print(f"  {ins.offset:04x}  {op:16} {ins.operand}{extra}")
