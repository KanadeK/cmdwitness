# Security policy

## Supported versions

The latest tagged release receives security fixes.

## Report a vulnerability

Please use GitHub's private vulnerability reporting for this repository. Do
not open a public issue containing an exploit, private path, token, or command
output that may expose user data.

Include the affected version, platform, smallest safe reproduction, impact,
and any proposed mitigation. You should receive an acknowledgement within
seven days.

## Execution boundary

CmdWitness intentionally executes the local baseline and candidate programs
named by the user. It passes argument arrays directly without a shell, uses
separate temporary directories, closes stdin unless declared, reduces inherited
environment, and applies time/output/file limits.

This is not an OS sandbox. Never use CmdWitness as the only isolation layer for
an untrusted executable. Run untrusted programs in a container or virtual
machine.

Reports may contain stdout, stderr, JSON values, and short file previews. Review
reports before posting them publicly.
