# Andromeda Deployment

This crate allows for a one-stop option for building and deploying aOS on a desired chain. The process is done in several steps:

1. Verify the provided setup
2. Build all required/selected contracts
3. Deploy the Operating System smart contracts
4. Upload and publish all required/selected ADOs

Compatible chains can be found in `/src/chains`.

## Prerequisites

Before you begin, ensure you have the following installed:

- Rust (latest stable version)
- Cargo (latest stable version)
- Docker

| Environment Variable        | Required | Description                                                                |
| --------------------------- | -------- | -------------------------------------------------------------------------- |
| `DEPLOYMENT_CHAIN`          | Yes      | Specifies the target chain for deployment                                  |
| `DEPLOYMENT_KERNEL_ADDRESS` | Yes\*    | Specifies the kernel contract address (\*not required if `DEPLOY_OS=true`) |
| `DEPLOY_OS`                 | No       | Boolean flag to indicate whether to undergo step 3                         |
| `DEPLOY_CONTRACTS`          | No       | Comma-separated list of contracts to deploy                                |

## Steps to Build

1. **Clone the repository:**

   ```sh
   git clone https://github.com/andromedaprotocol/andromeda-core.git
   cd andromeda-core
   ```

2. **Build the project:**

   ```sh
    cargo build -p andromeda-deploy --release
   ```

## Notes

- If deploying to an already deployed operating system only updated ADOs can be deployed (based on versions)
- If `DEPLOY_OS=true` and no kernel address is provided then a new OS will be created
