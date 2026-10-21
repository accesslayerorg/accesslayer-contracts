# Bonding Curve Guide and Price Prediction Off-Chain

This document explains the bonding curve formulas implemented in the contract, their constants, and how to predict prices off-chain.

## Bonding Curve Formulas

The contract supports three curve presets: **Flat**, **Linear**, and **Quadratic**.

### 1. Flat Curve
The price remains constant regardless of the supply.
$$P(s) = \text{base\_price}$$

### 2. Linear Curve
$$P(s) = \text{base\_price} + (\text{slope} \times s)$$
Where $s$ is the supply.

### 3. Quadratic Curve
$$P(s) = \text{base\_price} + (\text{slope} \times s^2)$$
Where $s$ is the supply.

---

## Formula Constants

- **Base Price (`base_price`)**: Stored in persistent storage. The starting price of the first key (when supply is 0).
- **Slope (`slope`)**: Global curve slope parameter (retrieved via `read_curve_slope`). Determines how fast the price changes.
- **Supply ($s$)**: The number of keys currently in circulation.

---

## Worked Example: Supply 0 → 1 (Buy First Key)

Suppose:
- $\text{base\_price} = 1000$ stroops
- $\text{slope} = 10$
- $\text{preset} = \text{Linear}$

Since the supply $s = 0$ before the purchase:
$$P(0) = 1000 + (10 \times 0) = 1000$$

### Fee split:
With a 90/10 split (`creator_bps = 9000`, `protocol_bps = 1000`):
$$\text{protocol\_fee} = \lfloor 1000 \times 1000 / 10000 \rfloor = 100$$
$$\text{creator\_fee} = 1000 - 100 = 900$$

### Total paid by buyer:
$$\text{total\_amount} = 1000 + 900 + 100 = 2000 \text{ stroops}$$

---

## TypeScript Price Prediction Snippet

Below is a TypeScript snippet to predict bonding curve prices off-chain.

```typescript
export enum CurvePreset {
  Flat = 0,
  Linear = 1,
  Quadratic = 2,
}

export interface FeeConfig {
  creatorBps: number;
  protocolBps: number;
}

export interface Quote {
  price: bigint;
  creatorFee: bigint;
  protocolFee: bigint;
  totalAmount: bigint;
}

export function getBuyQuote(
  basePrice: bigint,
  slope: bigint,
  supply: number,
  preset: CurvePreset,
  feeConfig: FeeConfig
): Quote {
  let price = basePrice;
  const s = BigInt(supply);

  if (preset === CurvePreset.Linear) {
    price = basePrice + slope * s;
  } else if (preset === CurvePreset.Quadratic) {
    price = basePrice + slope * s * s;
  }

  const protocolFee = (price * BigInt(feeConfig.protocolBps)) / 10000n;
  const creatorFee = price - protocolFee;
  const totalAmount = price + creatorFee + protocolFee;

  return {
    price,
    creatorFee,
    protocolFee,
    totalAmount,
  };
}

export function getSellQuote(
  basePrice: bigint,
  slope: bigint,
  supply: number, // Supply BEFORE selling (i.e. sell-path from supply -> supply - 1)
  preset: CurvePreset,
  feeConfig: FeeConfig
): Quote {
  if (supply <= 0) {
    throw new Error("SellUnderflow");
  }
  
  let price = basePrice;
  const s = BigInt(supply - 1); // Selling back uses previous supply step

  if (preset === CurvePreset.Linear) {
    price = basePrice + slope * s;
  } else if (preset === CurvePreset.Quadratic) {
    price = basePrice + slope * s * s;
  }

  const protocolFee = (price * BigInt(feeConfig.protocolBps)) / 10000n;
  const creatorFee = price - protocolFee;
  const totalAmount = price - creatorFee - protocolFee;

  return {
    price,
    creatorFee,
    protocolFee,
    totalAmount,
  };
}
```

---

## Precision and Rounding Considerations

- **Integer Arithmetic**: The contract uses integer division which discards any fractional part (equivalent to a floor towards zero).
- **Fee Split**: The protocol fee is computed first using integer division. The remainder of the price is given to the creator:
  $$\text{creator\_fee} = \text{price} - \text{protocol\_fee}$$
  This ensures no rounding dust is lost and the sum of fees always equals the price exactly.
