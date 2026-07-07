package token

import (
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/domain"
)

// RS256Signer issues and validates admin access JWTs.
type RS256Signer struct {
	private *rsa.PrivateKey
	public  *rsa.PublicKey
	ttl     time.Duration
	role    string
}

// NewRS256Signer loads PEM keys from disk.
func NewRS256Signer(privatePath, publicPath string, ttl time.Duration, role string) (*RS256Signer, error) {
	privPEM, err := os.ReadFile(privatePath)
	if err != nil {
		return nil, fmt.Errorf("read jwt private key: %w", err)
	}
	pubPEM, err := os.ReadFile(publicPath)
	if err != nil {
		return nil, fmt.Errorf("read jwt public key: %w", err)
	}
	priv, err := parsePrivateKey(privPEM)
	if err != nil {
		return nil, err
	}
	pub, err := parsePublicKey(pubPEM)
	if err != nil {
		return nil, err
	}
	return &RS256Signer{private: priv, public: pub, ttl: ttl, role: role}, nil
}

// SignAccessToken builds a short-lived JWT for an admin session.
func (s *RS256Signer) SignAccessToken(adminID uuid.UUID, mfaVerified bool) (string, int, error) {
	now := time.Now()
	exp := now.Add(s.ttl)
	claims := jwt.MapClaims{
		"sub":         adminID.String(),
		"role":        s.role,
		"mfaVerified": mfaVerified,
		"iat":         now.Unix(),
		"exp":         exp.Unix(),
	}
	tok := jwt.NewWithClaims(jwt.SigningMethodRS256, claims)
	signed, err := tok.SignedString(s.private)
	if err != nil {
		return "", 0, err
	}
	return signed, int(s.ttl.Seconds()), nil
}

// ParseAccessToken validates a bearer JWT and returns session claims.
func (s *RS256Signer) ParseAccessToken(raw string) (domain.SessionClaims, error) {
	parsed, err := jwt.Parse(raw, func(t *jwt.Token) (any, error) {
		if t.Method.Alg() != jwt.SigningMethodRS256.Alg() {
			return nil, fmt.Errorf("unexpected alg %s", t.Method.Alg())
		}
		return s.public, nil
	}, jwt.WithValidMethods([]string{jwt.SigningMethodRS256.Alg()}))
	if err != nil || !parsed.Valid {
		return domain.SessionClaims{}, fmt.Errorf("invalid jwt: %w", err)
	}
	claims, ok := parsed.Claims.(jwt.MapClaims)
	if !ok {
		return domain.SessionClaims{}, fmt.Errorf("invalid claims")
	}
	sub, _ := claims["sub"].(string)
	id, err := uuid.Parse(sub)
	if err != nil {
		return domain.SessionClaims{}, err
	}
	role, _ := claims["role"].(string)
	mfa, _ := claims["mfaVerified"].(bool)
	return domain.SessionClaims{AdminID: id, Role: role, MFAVerified: mfa}, nil
}

func parsePrivateKey(pemBytes []byte) (*rsa.PrivateKey, error) {
	block, _ := pem.Decode(pemBytes)
	if block == nil {
		return nil, fmt.Errorf("invalid private pem")
	}
	key, err := x509.ParsePKCS1PrivateKey(block.Bytes)
	if err == nil {
		return key, nil
	}
	pk, err2 := x509.ParsePKCS8PrivateKey(block.Bytes)
	if err2 != nil {
		return nil, fmt.Errorf("parse private key: %w", err)
	}
	rsaKey, ok := pk.(*rsa.PrivateKey)
	if !ok {
		return nil, fmt.Errorf("private key is not rsa")
	}
	return rsaKey, nil
}

func parsePublicKey(pemBytes []byte) (*rsa.PublicKey, error) {
	block, _ := pem.Decode(pemBytes)
	if block == nil {
		return nil, fmt.Errorf("invalid public pem")
	}
	pub, err := x509.ParsePKIXPublicKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("parse public key: %w", err)
	}
	rsaPub, ok := pub.(*rsa.PublicKey)
	if !ok {
		return nil, fmt.Errorf("public key is not rsa")
	}
	return rsaPub, nil
}
