import { execSync } from 'child_process'
import { readFileSync } from 'fs';

const versions = JSON.parse(readFileSync(new URL('./versions.json', import.meta.url), 'utf-8'));

versions.forEach((version) => {
  const command = `git archive heads/${version} docs/main | tar -x -C docs/ --transform "s|docs/main|${version}|"`
  console.log(`Fetched docs ${version}`)
  execSync(command, { stdio: 'inherit', shell: 'bash' })
})
