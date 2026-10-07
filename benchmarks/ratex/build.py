"""Build the isolated driver against a pinned upstream checkout."""
import json,pathlib,shutil,subprocess,sys
here=pathlib.Path(__file__).resolve().parent
source=pathlib.Path(sys.argv[1]).resolve()
output=pathlib.Path(sys.argv[2]).resolve()
expected='776c1d37bafa3bf445a0ab9377c55fe77f7a0133'
actual=subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()
if actual!=expected:raise SystemExit(f'Expected RaTeX {expected}; got {actual}')
(output/'src').mkdir(parents=True,exist_ok=True)
shutil.copyfile(here/'driver.rs',output/'src/main.rs')
shutil.copyfile(here/'Cargo.lock',output/'Cargo.lock')
manifest='[package]\nname="termitex-ratex-driver"\nversion="0.0.0"\nedition="2021"\n[dependencies]\n'
for name in ['ratex-render','ratex-layout','ratex-parser','ratex-types']:
    manifest+=name+'={path='+json.dumps(str(source/'crates'/name))+(',features=["embed-fonts"]' if name=='ratex-render' else '')+'}\n'
manifest+='serde_json="1"\nbase64="0.22"\n[profile.release]\nlto=true\ncodegen-units=1\npanic="abort"\n'
(output/'Cargo.toml').write_text(manifest)
subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(output/'Cargo.toml')],check=True)
print(output/'target/release/termitex-ratex-driver')
