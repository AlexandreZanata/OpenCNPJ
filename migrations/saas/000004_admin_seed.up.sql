-- Single admin placeholder; password and TOTP set via cmd/admin-bootstrap on first deploy.
INSERT INTO admin_users (id, email, password_hash, mfa_enabled)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'admin@opencnpj.local',
    '\x00',
    false
)
ON CONFLICT (email) DO NOTHING;
